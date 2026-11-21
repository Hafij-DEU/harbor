//! harbor-keymapd: the engine behind Harbor Settings' Keyboard Manager.
//!
//! Runs as root (systemd). It takes every keyboard for itself (EVIOCGRAB),
//! applies the rules in /etc/harbor/keymap.conf and types the result on its
//! own virtual keyboard ("Harbor keymap", uinput). When the rules change it
//! exits and systemd starts it again with the new rules. Plain std + libc.

#[path = "../keymap.rs"]
#[allow(dead_code)]
mod keymap;

use keymap::{mod_bit, Combo, Keymap, Kind, MODS};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const NAME: &str = "Harbor keymap";

const READ: u64 = 2;
const WRITE: u64 = 1;
const fn ioc(dir: u64, ty: u8, nr: u8, size: usize) -> u64 {
    (dir << 30) | ((size as u64) << 16) | ((ty as u64) << 8) | nr as u64
}
const KEY_BYTES: usize = 96; // KEY_MAX 0x2ff
const EVIOCGNAME: u64 = ioc(READ, b'E', 0x06, 256);
const EVIOCGKEY: u64 = ioc(READ, b'E', 0x18, KEY_BYTES);
const EVIOCGBIT_KEY: u64 = ioc(READ, b'E', 0x20 + EV_KEY as u8, KEY_BYTES);
const EVIOCGRAB: u64 = ioc(WRITE, b'E', 0x90, 4);
const UI_SET_EVBIT: u64 = ioc(WRITE, b'U', 100, 4);
const UI_SET_KEYBIT: u64 = ioc(WRITE, b'U', 101, 4);
const UI_DEV_SETUP: u64 = ioc(WRITE, b'U', 3, std::mem::size_of::<UinputSetup>());
const UI_DEV_CREATE: u64 = ioc(0, b'U', 1, 0);

#[repr(C)]
struct UinputSetup {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

enum Msg {
    Key(u16, i32),
    Gone(PathBuf),
}

fn mtime() -> Option<SystemTime> {
    std::fs::metadata(keymap::CONFIG)
        .and_then(|m| m.modified())
        .ok()
}

fn bit(bits: &[u8], code: u16) -> bool {
    bits.get(code as usize / 8)
        .is_some_and(|b| b & (1 << (code % 8)) != 0)
}

/// A keyboard's name, or None for anything that is not a keyboard.
fn keyboard_name(f: &File) -> Option<String> {
    let fd = f.as_raw_fd();
    let mut bits = [0u8; KEY_BYTES];
    let mut name = [0u8; 256];
    unsafe {
        if libc::ioctl(fd, EVIOCGBIT_KEY as _, bits.as_mut_ptr()) < 0
            || libc::ioctl(fd, EVIOCGNAME as _, name.as_mut_ptr()) < 0
        {
            return None;
        }
    }
    // A, Z, Space and Enter: a real keyboard, not a power button or mouse.
    if ![30, 44, 57, 28].iter().all(|&c| bit(&bits, c)) {
        return None;
    }
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    Some(String::from_utf8_lossy(&name[..end]).into_owned())
}

/// Whether a key is down right now (taking the keyboard over mid-press
/// would leave that key stuck for the desktop).
fn any_key_down(f: &File) -> bool {
    let mut state = [0u8; KEY_BYTES];
    let ok = unsafe { libc::ioctl(f.as_raw_fd(), EVIOCGKEY as _, state.as_mut_ptr()) } >= 0;
    ok && state.iter().any(|&b| b != 0)
}

/// Take any keyboard not taken yet; each gets a reading thread.
fn scan(tx: &mpsc::Sender<Msg>, known: &mut HashSet<PathBuf>) {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        return;
    };
    for entry in dir.flatten() {
        let path = entry.path();
        let is_event = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("event"));
        if !is_event || known.contains(&path) {
            continue;
        }
        let Ok(mut f) = File::open(&path) else {
            continue;
        };
        match keyboard_name(&f) {
            Some(name) if name != NAME => {}
            _ => continue,
        }
        let fd = f.as_raw_fd();
        if unsafe { libc::ioctl(fd, EVIOCGRAB as _, 1 as libc::c_int) } < 0 {
            continue; // someone else holds it; try again on the next scan
        }
        if any_key_down(&f) {
            unsafe { libc::ioctl(fd, EVIOCGRAB as _, 0 as libc::c_int) };
            continue; // a key is down: try again on the next scan
        }
        known.insert(path.clone());
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 24 * 64];
            loop {
                let n = match f.read(&mut buf) {
                    Ok(n) if n > 0 => n,
                    _ => {
                        let _ = tx.send(Msg::Gone(path));
                        return;
                    }
                };
                for ev in buf[..n].chunks_exact(24) {
                    let kind = u16::from_ne_bytes([ev[16], ev[17]]);
                    let code = u16::from_ne_bytes([ev[18], ev[19]]);
                    let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
                    if kind == EV_KEY && tx.send(Msg::Key(code, value)).is_err() {
                        return;
                    }
                }
            }
        });
    }
}

fn create_output() -> std::io::Result<File> {
    let f = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/uinput")?;
    let fd = f.as_raw_fd();
    let mut setup = UinputSetup {
        bustype: 0x06, // BUS_VIRTUAL
        vendor: 0x1209,
        product: 0x4b4d,
        version: 1,
        name: [0; 80],
        ff_effects_max: 0,
    };
    setup.name[..NAME.len()].copy_from_slice(NAME.as_bytes());
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
    Ok(f)
}

/// What a pressed source key turned into, to undo on release.
struct Active {
    to: Combo,
    added_mods: Vec<u16>,
    /// Modifiers lifted for a shortcut, pressed again on release if still held.
    lifted: Vec<u16>,
}

struct Engine {
    out: File,
    keys: HashMap<u16, Combo>,
    shortcuts: Vec<(Combo, Combo)>,
    down: HashSet<u16>,
    active: HashMap<u16, Active>,
}

impl Engine {
    fn emit(&mut self, code: u16, value: i32) {
        let mut ev = [0u8; 48];
        ev[16..18].copy_from_slice(&EV_KEY.to_ne_bytes());
        ev[18..20].copy_from_slice(&code.to_ne_bytes());
        ev[20..24].copy_from_slice(&value.to_ne_bytes());
        // ev[24..48] stays zero: SYN_REPORT
        let _ = self.out.write_all(&ev);
        match value {
            1 => {
                self.down.insert(code);
            }
            0 => {
                self.down.remove(&code);
            }
            _ => {}
        }
    }

    fn mods(&self) -> u8 {
        self.down.iter().fold(0, |m, &c| m | mod_bit(c))
    }

    fn press(&mut self, to: Combo) -> Vec<u16> {
        let mut added = Vec::new();
        for (bit, _, code) in MODS {
            if to.mods & bit != 0 && self.mods() & bit == 0 {
                self.emit(code, 1);
                added.push(code);
            }
        }
        if to.key != 0 {
            self.emit(to.key, 1);
        }
        added
    }

    fn key(&mut self, code: u16, value: i32) {
        match value {
            1 => {
                let mods = self.mods();
                let hit = (mod_bit(code) == 0)
                    .then(|| {
                        self.shortcuts
                            .iter()
                            .find(|(from, _)| from.key == code && from.mods == mods)
                            .map(|(_, to)| *to)
                    })
                    .flatten();
                if let Some(to) = hit {
                    let lifted: Vec<u16> = self
                        .down
                        .iter()
                        .copied()
                        .filter(|&c| mod_bit(c) != 0)
                        .collect();
                    for &m in &lifted {
                        self.emit(m, 0);
                    }
                    let added_mods = self.press(to);
                    self.active.insert(
                        code,
                        Active {
                            to,
                            added_mods,
                            lifted,
                        },
                    );
                } else if let Some(&to) = self.keys.get(&code) {
                    let added_mods = self.press(to);
                    self.active.insert(
                        code,
                        Active {
                            to,
                            added_mods,
                            lifted: Vec::new(),
                        },
                    );
                } else {
                    self.emit(code, 1);
                }
            }
            2 => match self.active.get(&code).map(|a| a.to.key) {
                Some(0) => {}
                Some(key) => self.emit(key, 2),
                None => self.emit(code, 2),
            },
            _ => {
                if let Some(a) = self.active.remove(&code) {
                    if a.to.key != 0 {
                        self.emit(a.to.key, 0);
                    }
                    for m in a.added_mods {
                        self.emit(m, 0);
                    }
                    for m in a.lifted {
                        self.emit(m, 1);
                    }
                    return;
                }
                // A modifier let go while its shortcut is still down.
                for a in self.active.values_mut() {
                    a.lifted.retain(|&m| m != code);
                }
                if self.down.contains(&code) {
                    self.emit(code, 0);
                }
            }
        }
    }
}

fn main() {
    let stamp = mtime();
    let map = Keymap::load();
    let changed = || mtime() != stamp;
    if !map.enabled || map.rules.is_empty() {
        // Nothing to do: wait for rules without touching any keyboard.
        loop {
            std::thread::sleep(Duration::from_secs(2));
            if changed() {
                return;
            }
        }
    }
    let out = match create_output() {
        Ok(f) => f,
        Err(err) => {
            eprintln!("harbor-keymapd: cannot create the virtual keyboard: {err}");
            std::process::exit(1);
        }
    };
    let mut engine = Engine {
        out,
        keys: HashMap::new(),
        shortcuts: Vec::new(),
        down: HashSet::new(),
        active: HashMap::new(),
    };
    for r in &map.rules {
        match r.kind {
            Kind::Key => {
                engine.keys.insert(r.from.key, r.to);
            }
            Kind::Shortcut => engine.shortcuts.push((r.from, r.to)),
        }
    }
    // Let the new virtual keyboard settle before keys arrive.
    std::thread::sleep(Duration::from_millis(300));
    let (tx, rx) = mpsc::channel();
    let mut known = HashSet::new();
    scan(&tx, &mut known);
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Msg::Key(code, value)) => engine.key(code, value),
            Ok(Msg::Gone(path)) => {
                known.remove(&path);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if last.elapsed() >= Duration::from_secs(2) {
            last = Instant::now();
            if changed() && engine.down.is_empty() {
                return; // systemd starts us again with the new rules
            }
            scan(&tx, &mut known); // keyboards plugged in later
        }
    }
}
