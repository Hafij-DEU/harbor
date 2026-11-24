//! Raw Linux input: opening keyboards (/dev/input/event*), grabbing them,
//! reading their key state, and the virtual output keyboard (/dev/uinput).
//! Every `unsafe` ioctl of the service is in this file.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use crate::keyboard::devices::VIRTUAL_NAME;

pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
/// Size of one `struct input_event` on 64-bit Linux.
pub const EVENT_SIZE: usize = 24;

const READ: u64 = 2;
const WRITE: u64 = 1;
const fn ioc(dir: u64, ty: u8, nr: u8, size: usize) -> u64 {
    (dir << 30) | ((size as u64) << 16) | ((ty as u64) << 8) | nr as u64
}
/// Bytes for a key bitmap (KEY_MAX 0x2ff).
const KEY_BYTES: usize = 96;
const EVIOCGNAME: u64 = ioc(READ, b'E', 0x06, 256);
const EVIOCGID: u64 = ioc(READ, b'E', 0x02, 8);
const EVIOCGKEY: u64 = ioc(READ, b'E', 0x18, KEY_BYTES);
const EVIOCGBIT_KEY: u64 = ioc(READ, b'E', 0x20 + EV_KEY as u8, KEY_BYTES);
const EVIOCGRAB: u64 = ioc(WRITE, b'E', 0x90, 4);
const UI_SET_EVBIT: u64 = ioc(WRITE, b'U', 100, 4);
const UI_SET_KEYBIT: u64 = ioc(WRITE, b'U', 101, 4);
const UI_DEV_SETUP: u64 = ioc(WRITE, b'U', 3, std::mem::size_of::<UinputSetup>());
const UI_DEV_CREATE: u64 = ioc(0, b'U', 1, 0);

/// A physical keyboard opened for reading.
pub struct Device {
    pub path: PathBuf,
    pub file: File,
    pub name: String,
    /// "vvvv:pppp"
    pub id: String,
    pub bus: u16,
}

/// Open an event node if it is a real keyboard (has A, Z, Space and Enter),
/// not a power button, mouse or Harbor's own virtual keyboard.
pub fn open_keyboard(path: &Path) -> Option<Device> {
    let file = File::open(path).ok()?;
    let fd = file.as_raw_fd();
    let mut bits = [0u8; KEY_BYTES];
    let mut name = [0u8; 256];
    let mut id = [0u16; 4]; // bus, vendor, product, version
    let ok = unsafe {
        libc::ioctl(fd, EVIOCGBIT_KEY as _, bits.as_mut_ptr()) >= 0
            && libc::ioctl(fd, EVIOCGNAME as _, name.as_mut_ptr()) >= 0
            && libc::ioctl(fd, EVIOCGID as _, id.as_mut_ptr()) >= 0
    };
    if !ok || ![30u16, 44, 57, 28].iter().all(|&c| bit(&bits, c)) {
        return None;
    }
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    let name = String::from_utf8_lossy(&name[..end]).into_owned();
    if name == VIRTUAL_NAME {
        return None;
    }
    Some(Device {
        path: path.to_owned(),
        file,
        name,
        id: format!("{:04x}:{:04x}", id[1], id[2]),
        bus: id[0],
    })
}

/// Take (true) or give back (false) exclusive use of a keyboard.
pub fn grab(file: &File, on: bool) -> bool {
    unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB as _, on as libc::c_int) >= 0 }
}

/// Keys the kernel considers down on this keyboard right now.
pub fn held_keys(file: &File) -> Vec<u16> {
    let mut state = [0u8; KEY_BYTES];
    if unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGKEY as _, state.as_mut_ptr()) } < 0 {
        return Vec::new();
    }
    (0..(KEY_BYTES * 8) as u16)
        .filter(|&c| bit(&state, c))
        .collect()
}

fn bit(bits: &[u8], code: u16) -> bool {
    bits.get(code as usize / 8)
        .is_some_and(|b| b & (1 << (code % 8)) != 0)
}

/// (type, code, value) of one raw event.
pub fn decode(ev: &[u8]) -> (u16, u16, i32) {
    (
        u16::from_ne_bytes([ev[16], ev[17]]),
        u16::from_ne_bytes([ev[18], ev[19]]),
        i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]),
    )
}

#[repr(C)]
struct UinputSetup {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

/// The virtual keyboard ("Harbor keymap") that grabbed keyboards type through.
pub struct VirtualKeyboard {
    file: File,
}

impl VirtualKeyboard {
    pub fn create() -> std::io::Result<VirtualKeyboard> {
        let file = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")?;
        let fd = file.as_raw_fd();
        let mut setup = UinputSetup {
            bustype: 0x06, // BUS_VIRTUAL
            vendor: 0x1209,
            product: 0x4b4d,
            version: 1,
            name: [0; 80],
            ff_effects_max: 0,
        };
        setup.name[..VIRTUAL_NAME.len()].copy_from_slice(VIRTUAL_NAME.as_bytes());
        unsafe {
            libc::ioctl(fd, UI_SET_EVBIT as _, EV_KEY as libc::c_int);
            libc::ioctl(fd, UI_SET_EVBIT as _, EV_SYN as libc::c_int);
            // Keyboard keys only (0x100-0x15f are mouse and joystick buttons).
            for code in (1..=0xffu16).chain(0x160..=0x2ff) {
                libc::ioctl(fd, UI_SET_KEYBIT as _, code as libc::c_int);
            }
            if libc::ioctl(fd, UI_DEV_SETUP as _, &setup as *const UinputSetup) < 0
                || libc::ioctl(fd, UI_DEV_CREATE as _) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        // Let the desktop pick the new device up before keys arrive.
        std::thread::sleep(std::time::Duration::from_millis(300));
        Ok(VirtualKeyboard { file })
    }

    /// One key event followed by SYN_REPORT.
    pub fn key(&mut self, code: u16, value: i32) {
        let mut ev = [0u8; EVENT_SIZE * 2];
        ev[16..18].copy_from_slice(&EV_KEY.to_ne_bytes());
        ev[18..20].copy_from_slice(&code.to_ne_bytes());
        ev[20..24].copy_from_slice(&value.to_ne_bytes());
        // The second event stays all zero: EV_SYN / SYN_REPORT.
        let _ = self.file.write_all(&ev);
    }
}
