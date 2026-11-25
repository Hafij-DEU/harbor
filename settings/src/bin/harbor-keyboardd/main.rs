//! harbor-keyboardd: the root service behind Harbor Settings' Keyboard Manager.
//!
//! Read `src/keyboard/mod.rs` first for the big picture. This service:
//! 1. opens every keyboard and reads its keys (one thread per keyboard;
//!    with input-remapper running, its "forwarded" copy is read as well);
//! 2. grabs a keyboard only when it has to change keys (remap rules or a
//!    disabled stuck key); grabbed keys are typed again on the virtual
//!    keyboard by `remap.rs`. Other keyboards are only watched;
//! 3. turns keyboards off and on with sysfs `inhibited` (`sysfs.rs`), but
//!    never turns off the last keyboard, so the user can always log in;
//! 4. runs the stuck-key guard (`guard.rs`) and restarts a keyboard or
//!    disables the stuck key, with a desktop notification (`notify.rs`);
//! 5. writes `/run/harbor-keyboard/status` and, when asked, `keycheck`.
//!
//! The config file is checked every half second and reloaded in place.
//! On SIGTERM every keyboard it turned off is turned on again.

#[path = "../../keyboard/mod.rs"]
#[allow(dead_code)]
mod keyboard;

mod evdev;
mod guard;
mod notify;
mod remap;
mod sysfs;

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use keyboard::config::{self, Config, StuckAction};
use keyboard::devices::Keyboard;
use keyboard::keys;
use keyboard::status::{self, BoardStatus, KeyCheck, Status, StuckEvent};

/// How long the main loop waits for a key before doing housekeeping.
const WAIT: Duration = Duration::from_millis(100);
/// Config, guard and status are checked this often.
const TICK: Duration = Duration::from_millis(500);
/// New keyboards are looked for this often.
const SCAN_EVERY: Duration = Duration::from_secs(2);
/// Sysfs dirs of keyboards this service turned off, so a restarted service
/// can turn them on again (and never touches ones another app turned off).
const OFF_RECORD: &str = "/run/harbor-keyboard/off";
const KEEP_EVENTS: usize = 10;
const KEEP_PRESSES: usize = 30;

static STOP: AtomicBool = AtomicBool::new(false);

enum Msg {
    Key { board: u64, code: u16, value: i32 },
    Gone { board: u64 },
}

/// One open keyboard.
struct Board {
    dev: evdev::Device,
    kb: Keyboard,
    /// Its /sys/devices/.../inputN dir (for `inhibited`).
    sysfs: Option<PathBuf>,
    grabbed: bool,
    /// Turned off by this service.
    off: bool,
    /// Set to off but kept on: it is the only keyboard.
    kept_on: bool,
    /// Keys disabled after sticking, until the service restarts.
    disabled: HashSet<u16>,
    watch: guard::Watch,
}

struct Service {
    config: Config,
    config_stamp: Option<SystemTime>,
    boards: BTreeMap<u64, Board>,
    known: HashSet<PathBuf>,
    next_id: u64,
    tx: mpsc::Sender<Msg>,
    remapper: Option<remap::Remapper>,
    irq: Option<u64>,
    events: Vec<StuckEvent>,
    /// Key check: on while Settings asks; presses kept only then.
    keycheck: Option<u32>,
    presses: VecDeque<(u64, String, u16)>,
    keycheck_dirty: bool,
    last_status: String,
}

fn main() {
    let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
    }
    let _ = fs::create_dir_all(status::DIR);
    let _ = fs::set_permissions(status::DIR, fs::Permissions::from_mode(0o755));
    turn_on_recorded();

    let (tx, rx) = mpsc::channel();
    let mut svc = Service {
        config: Config::load(),
        config_stamp: config_stamp(),
        boards: BTreeMap::new(),
        known: HashSet::new(),
        next_id: 0,
        tx,
        remapper: None,
        irq: sysfs::i8042_interrupts(),
        events: Vec::new(),
        keycheck: None,
        presses: VecDeque::new(),
        keycheck_dirty: false,
        last_status: String::new(),
    };
    svc.scan();
    svc.apply();

    let (mut last_tick, mut last_scan) = (Instant::now(), Instant::now());
    while !STOP.load(Ordering::Relaxed) {
        match rx.recv_timeout(WAIT) {
            Ok(Msg::Key { board, code, value }) => svc.on_key(board, code, value),
            Ok(Msg::Gone { board }) => svc.remove(board),
            Err(_) => {}
        }
        if svc.keycheck.is_some() && svc.keycheck_dirty {
            svc.write_keycheck();
        }
        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            if last_scan.elapsed() >= SCAN_EVERY {
                last_scan = Instant::now();
                if svc.scan() {
                    svc.apply();
                }
            }
            svc.tick();
        }
    }
    svc.shutdown();
}

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

fn config_stamp() -> Option<SystemTime> {
    [config::PATH, config::OLD_PATH]
        .iter()
        .find_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Turn on keyboards a previous run turned off; `apply` turns off again
/// whatever the config still wants off.
fn turn_on_recorded() {
    for dir in fs::read_to_string(OFF_RECORD).unwrap_or_default().lines() {
        sysfs::set_inhibited(Path::new(dir), false);
    }
    let _ = fs::remove_file(OFF_RECORD);
}

/// Write a file atomically (tmp + rename) with a mode and owner.
fn write_file(path: &str, text: &str, mode: u32, owner: Option<u32>) {
    let tmp = format!("{path}.tmp");
    if fs::write(&tmp, text).is_err() {
        return;
    }
    let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(mode));
    if let Some(uid) = owner {
        let _ = std::os::unix::fs::chown(&tmp, Some(uid), None);
    }
    let _ = fs::rename(&tmp, path);
}

impl Service {
    // ------------------------------------------------------------ keyboards

    /// Open keyboards that appeared since the last scan. True when any did.
    fn scan(&mut self) -> bool {
        let Ok(dir) = fs::read_dir("/dev/input") else {
            return false;
        };
        let mut added = false;
        for entry in dir.flatten() {
            let path = entry.path();
            let is_event = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("event"));
            if !is_event || self.known.contains(&path) {
                continue;
            }
            let Some(dev) = evdev::open_keyboard(&path) else {
                continue;
            };
            // Virtual keyboards only repeat a real one: skip them, except
            // input-remapper's "forwarded" copy. input-remapper takes the real
            // keyboard for itself, so its keys only show up on that copy; the
            // real keyboard is still the one turned off or restarted.
            let dir = sysfs::input_dir(&path);
            let is_virtual = dir
                .as_ref()
                .is_none_or(|d| d.starts_with("/sys/devices/virtual"));
            let sysfs = if !is_virtual {
                dir
            } else if is_forwarded_copy(&dev.name) {
                sysfs::real_input_dir(&dev.id, dev.bus)
            } else {
                continue;
            };
            let Ok(reader) = dev.file.try_clone() else {
                continue;
            };
            let id = self.next_id;
            self.next_id += 1;
            spawn_reader(reader, id, self.tx.clone());
            self.known.insert(path.clone());
            let kb = Keyboard {
                name: dev.name.clone(),
                id: dev.id.clone(),
                bus: dev.bus,
            };
            eprintln!(
                "harbor-keyboardd: watching {} ({}, {})",
                kb.name,
                kb.id,
                path.display()
            );
            self.boards.insert(
                id,
                Board {
                    sysfs,
                    dev,
                    kb,
                    grabbed: false,
                    off: false,
                    kept_on: false,
                    disabled: HashSet::new(),
                    watch: guard::Watch::default(),
                },
            );
            added = true;
        }
        added
    }

    fn remove(&mut self, id: u64) {
        if let Some(b) = self.boards.remove(&id) {
            self.known.remove(&b.dev.path);
            eprintln!("harbor-keyboardd: {} unplugged", b.kb.name);
        }
    }

    // ------------------------------------------------------------ config

    /// Bring every keyboard in line with the config: off/on, rules, grabs.
    fn apply(&mut self) {
        self.apply_off();
        if let Some(r) = &mut self.remapper {
            r.set_rules(self.config.active_rules());
        }
        self.update_grabs();
        self.write_status();
    }

    /// Turn keyboards off or on as the config says, but keep at least one on.
    fn apply_off(&mut self) {
        let all_off =
            !self.boards.is_empty() && self.boards.values().all(|b| self.config.is_off(&b.kb));
        let mut record = Vec::new();
        for b in self.boards.values_mut() {
            let wanted = self.config.is_off(&b.kb);
            let was_kept = b.kept_on;
            b.kept_on = wanted && all_off;
            let off = wanted && !all_off;
            if b.kept_on && !was_kept {
                notify::notify(&format!(
                    "{} is set to off, but it is the only keyboard, so it stays on.",
                    b.kb.name
                ));
            }
            if let Some(dir) = &b.sysfs {
                if off != b.off && sysfs::can_inhibit(dir) && sysfs::set_inhibited(dir, off) {
                    b.off = off;
                    eprintln!(
                        "harbor-keyboardd: {} turned {}",
                        b.kb.name,
                        if off { "off" } else { "on" }
                    );
                }
                if b.off {
                    record.push(dir.display().to_string());
                }
            }
        }
        if record.is_empty() {
            let _ = fs::remove_file(OFF_RECORD);
        } else {
            write_file(OFF_RECORD, &(record.join("\n") + "\n"), 0o644, None);
        }
    }

    /// Grab keyboards that need their keys changed; let go of the others.
    fn update_grabs(&mut self) {
        let rules = self.config.active_rules();
        for b in self.boards.values_mut() {
            let has_rules = rules.iter().any(|r| r.on.matches(&b.kb.id, b.kb.bus));
            let want = !b.off && (has_rules || !b.disabled.is_empty());
            if want && !b.grabbed {
                // Taking a keyboard mid-press would leave that key down for the desktop.
                if !evdev::held_keys(&b.dev.file).is_empty() {
                    continue;
                }
                if self.remapper.is_none() {
                    match evdev::VirtualKeyboard::create() {
                        Ok(out) => self.remapper = Some(remap::Remapper::new(out, rules.clone())),
                        Err(err) => {
                            eprintln!(
                                "harbor-keyboardd: cannot create the virtual keyboard: {err}"
                            );
                            return;
                        }
                    }
                }
                b.grabbed = evdev::grab(&b.dev.file, true);
            } else if !want && b.grabbed && self.remapper.as_ref().is_none_or(|r| r.idle()) {
                evdev::grab(&b.dev.file, false);
                b.grabbed = false;
            }
        }
    }

    // ------------------------------------------------------------ keys

    fn on_key(&mut self, id: u64, code: u16, value: i32) {
        let Some(b) = self.boards.get_mut(&id) else {
            return;
        };
        b.watch.event(code, value, Instant::now());
        if self.keycheck.is_some() {
            if value == 1 {
                self.presses.push_back((unix_ms(), b.kb.id.clone(), code));
                if self.presses.len() > KEEP_PRESSES {
                    self.presses.pop_front();
                }
            }
            self.keycheck_dirty = true;
        }
        if !b.grabbed || b.disabled.contains(&code) {
            return; // watched only, or a disabled key: nothing to type
        }
        if let Some(r) = &mut self.remapper {
            r.key(code, value, &b.kb.id, b.kb.bus);
        }
    }

    // ------------------------------------------------------------ housekeeping

    fn tick(&mut self) {
        let stamp = config_stamp();
        if stamp != self.config_stamp && self.remapper.as_ref().is_none_or(|r| r.idle()) {
            self.config_stamp = stamp;
            self.config = Config::load();
            eprintln!("harbor-keyboardd: settings reloaded");
            self.apply();
        } else {
            self.update_grabs(); // grabs that had to wait for a key to go up
        }
        self.guard();
        self.update_keycheck();
        self.write_status();
    }

    /// Look for stuck keys and act on them.
    fn guard(&mut self) {
        let now = Instant::now();
        let irq = sysfs::i8042_interrupts();
        let laptop_active = irq.is_some() && irq != self.irq;
        self.irq = irq;
        let mut found = Vec::new();
        for (&id, b) in self.boards.iter_mut() {
            let laptop = b.kb.is_internal();
            if laptop && laptop_active {
                b.watch.activity(now);
            }
            let action = self.config.stuck_for(&b.kb);
            // No i8042 counter: a held laptop key cannot be told from a stuck one.
            if action == StuckAction::Nothing || b.off || (laptop && irq.is_none()) {
                continue;
            }
            let limit = if laptop {
                guard::LIMIT_LAPTOP
            } else {
                guard::LIMIT_OTHER
            };
            let mut held = evdev::held_keys(&b.dev.file);
            // Keys left out by the "Keys that count" filter never trigger the guard.
            held.retain(|&c| self.config.stuck_keys.watches(c));
            let ignore: Vec<u16> = b.disabled.iter().copied().collect();
            if let Some(code) = b.watch.stuck_key(now, limit, &held, &ignore) {
                found.push((id, code, b.watch.decide(action, now)));
            }
        }
        for (id, code, decision) in found {
            self.handle_stuck(id, code, decision);
        }
    }

    fn handle_stuck(&mut self, id: u64, code: u16, decision: guard::Decision) {
        let Some(b) = self.boards.get_mut(&id) else {
            return;
        };
        let key = keys::any_label(code);
        let restarted = b.sysfs.as_deref().is_some_and(sysfs::restart);
        b.watch.clear(Instant::now());
        let action = match decision {
            guard::Decision::Restart => {
                notify::notify(&format!(
                    "{key} was stuck on {}. The keyboard was restarted.",
                    b.kb.name
                ));
                "restart"
            }
            guard::Decision::DisableKey => {
                // Release it on the virtual keyboard now: its own release
                // will be dropped from here on.
                if b.grabbed {
                    if let Some(r) = &mut self.remapper {
                        r.key(code, 0, &b.kb.id, b.kb.bus);
                    }
                }
                b.disabled.insert(code);
                notify::notify(&format!(
                    "{key} got stuck on {}, so it is disabled until restart. \
                     Turn it back on in Settings › Keyboard.",
                    b.kb.name
                ));
                "disable-key"
            }
        };
        if !restarted {
            eprintln!(
                "harbor-keyboardd: {} cannot be restarted (no sysfs inhibited)",
                b.kb.name
            );
        }
        self.events.push(StuckEvent {
            at: unix_ms() / 1000,
            id: b.kb.id.clone(),
            code,
            action: action.to_owned(),
        });
        if self.events.len() > KEEP_EVENTS {
            self.events.remove(0);
        }
        self.update_grabs();
    }

    // ------------------------------------------------------------ files for Settings

    fn write_status(&mut self) {
        let mut boards: Vec<BoardStatus> = self
            .boards
            .values()
            .map(|b| {
                let mut disabled: Vec<u16> = b.disabled.iter().copied().collect();
                disabled.sort_unstable();
                BoardStatus {
                    id: b.kb.id.clone(),
                    bus: b.kb.bus,
                    name: b.kb.name.clone(),
                    off: b.off,
                    kept_on: b.kept_on,
                    disabled,
                }
            })
            .collect();
        boards.sort_by(|a, b| a.name.cmp(&b.name));
        boards.dedup_by(|a, b| a.id == b.id);
        let text = Status {
            boards,
            events: self.events.clone(),
        }
        .text();
        if text != self.last_status {
            write_file(status::STATUS, &text, 0o644, None);
            self.last_status = text;
        }
    }

    /// Key check runs while the active user's request file is fresh.
    fn update_keycheck(&mut self) {
        let asking = sysfs::active_uid().filter(|&uid| {
            fs::symlink_metadata(status::keycheck_request(uid)).is_ok_and(|m| {
                m.is_file()
                    && m.uid() == uid
                    && m.modified()
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age < status::KEYCHECK_FRESH)
            })
        });
        if asking != self.keycheck {
            self.keycheck = asking;
            self.presses.clear();
            if asking.is_none() {
                let _ = fs::remove_file(status::KEYCHECK);
            }
        }
        if self.keycheck.is_some() {
            self.write_keycheck();
        }
    }

    fn write_keycheck(&mut self) {
        let now = Instant::now();
        let mut check = KeyCheck {
            held: Vec::new(),
            presses: self.presses.iter().cloned().collect(),
        };
        for b in self.boards.values() {
            for code in evdev::held_keys(&b.dev.file) {
                let ms = b
                    .watch
                    .held_for(code, now)
                    .map_or(0, |d| d.as_millis() as u64);
                // A keyboard and its input-remapper copy report the same key.
                match check
                    .held
                    .iter_mut()
                    .find(|h| h.0 == b.kb.id && h.1 == code)
                {
                    Some(h) => h.2 = h.2.max(ms),
                    None => check.held.push((b.kb.id.clone(), code, ms)),
                }
            }
        }
        write_file(status::KEYCHECK, &check.text(), 0o600, self.keycheck);
        self.keycheck_dirty = false;
    }

    // ------------------------------------------------------------ stop

    /// Leave every keyboard as it was before the service.
    fn shutdown(&mut self) {
        for b in self.boards.values_mut() {
            if b.grabbed {
                evdev::grab(&b.dev.file, false);
            }
            if b.off {
                if let Some(dir) = &b.sysfs {
                    sysfs::set_inhibited(dir, false);
                }
            }
        }
        let _ = fs::remove_file(OFF_RECORD);
        let _ = fs::remove_file(status::STATUS);
        let _ = fs::remove_file(status::KEYCHECK);
    }
}

/// input-remapper names its copy of a keyboard "input-remapper NAME forwarded".
fn is_forwarded_copy(name: &str) -> bool {
    name.starts_with("input-remapper ") && name.ends_with(" forwarded")
}

/// Read one keyboard's key events into the main loop until it goes away.
fn spawn_reader(mut file: fs::File, board: u64, tx: mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; evdev::EVENT_SIZE * 64];
        loop {
            let n = match file.read(&mut buf) {
                Ok(n) if n > 0 => n,
                _ => {
                    let _ = tx.send(Msg::Gone { board });
                    return;
                }
            };
            for ev in buf[..n].chunks_exact(evdev::EVENT_SIZE) {
                let (kind, code, value) = evdev::decode(ev);
                if kind == evdev::EV_KEY && tx.send(Msg::Key { board, code, value }).is_err() {
                    return;
                }
            }
        }
    });
}
