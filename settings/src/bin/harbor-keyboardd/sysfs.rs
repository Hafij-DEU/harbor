//! Kernel files outside /dev/input: turning a keyboard off and on (sysfs
//! `inhibited`), the i8042 interrupt counter used to tell a held key from a
//! stuck one, and which user sits at the screen.

use std::fs;
use std::path::{Path, PathBuf};

/// The sysfs input directory of an event node:
/// /dev/input/event3 -> /sys/class/input/event3/device (= .../input/input3).
pub fn input_dir(event_node: &Path) -> Option<PathBuf> {
    let name = event_node.file_name()?.to_str()?;
    fs::canonicalize(format!("/sys/class/input/{name}/device")).ok()
}

/// The real (not virtual) keyboard dir with this "vvvv:pppp" id and bus:
/// the keyboard behind an input-remapper copy, which is the one to turn off
/// or restart.
pub fn real_input_dir(id: &str, bus: u16) -> Option<PathBuf> {
    for entry in fs::read_dir("/sys/class/input").ok()?.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("input") {
            continue;
        }
        let Ok(dir) = fs::canonicalize(entry.path()) else {
            continue;
        };
        if dir.starts_with("/sys/devices/virtual") || !can_inhibit(&dir) {
            continue;
        }
        let read = |f: &str| {
            fs::read_to_string(dir.join("id").join(f))
                .unwrap_or_default()
                .trim()
                .to_owned()
        };
        let same_id = format!("{}:{}", read("vendor"), read("product")) == id;
        if same_id && u16::from_str_radix(&read("bustype"), 16).ok() == Some(bus) {
            return Some(dir);
        }
    }
    None
}

/// Whether the kernel can turn this keyboard off (Linux 5.11+).
pub fn can_inhibit(dir: &Path) -> bool {
    dir.join("inhibited").exists()
}

/// Turn a keyboard off (true) or on (false). Turning it off makes the
/// kernel release every key it holds down, which is what "restart" uses.
pub fn set_inhibited(dir: &Path, off: bool) -> bool {
    fs::write(dir.join("inhibited"), if off { "1" } else { "0" }).is_ok()
}

/// Off for a moment, then on again: every held key is released.
pub fn restart(dir: &Path) -> bool {
    if !set_inhibited(dir, true) {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    set_inhibited(dir, false)
}

/// Total i8042 keyboard interrupts (IRQ 1). It changes whenever the laptop
/// keyboard sends anything, including the repeats of a key really held, so
/// "key down but no interrupts" means the release was lost: a stuck key.
pub fn i8042_interrupts() -> Option<u64> {
    let text = fs::read_to_string("/proc/interrupts").ok()?;
    text.lines()
        .find(|l| l.trim_start().starts_with("1:") && l.trim_end().ends_with("i8042"))
        .map(|l| {
            l.split_whitespace()
                .skip(1)
                .filter_map(|w| w.parse::<u64>().ok())
                .sum()
        })
}

/// The user at the active seat (for Key check), from systemd-logind.
pub fn active_uid() -> Option<u32> {
    fs::read_to_string("/run/systemd/seats/seat0")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("ACTIVE_UID="))
        .and_then(|v| v.trim().parse().ok())
}
