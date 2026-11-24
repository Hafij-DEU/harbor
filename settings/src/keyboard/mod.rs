//! Keyboard Manager. Start reading here.
//!
//! Harbor Settings (runs as the user) edits the config; `harbor-keyboardd`
//! (runs as root under systemd) does the work. This module is shared by both:
//! the service includes it with `#[path]`, so it must stay plain std.
//!
//! What it does, for every keyboard or one keyboard:
//! * remap a key or shortcut to another key or shortcut, or disable a key;
//! * turn a keyboard off (sysfs `inhibited`, like Toggle Internal Keyboard);
//! * stuck-key guard: when a key stays down while the keyboard is silent,
//!   restart (reset) that keyboard, or disable the stuck key;
//! * Key check: show which keys each keyboard is pressing or holding.
//!
//! Where things live:
//! | File | Role |
//! |---|---|
//! | `keyboard/keys.rs` | key names, codes, labels, modifiers, `Combo` |
//! | `keyboard/devices.rs` | `Target` (every / laptop / one keyboard), `keyboards()` |
//! | `keyboard/config.rs` | `/etc/harbor/keyboard.conf` format: `Config` |
//! | `keyboard/status.rs` | files the service writes for Settings: `Status`, `KeyCheck` |
//! | `bin/harbor-keyboardd/` | the service (see its `main.rs` header) |
//! | `pages/keyboard/` | the Settings UI |
//! | `system/keyboard.rs` | Settings -> `pkexec harbor-keyboard-apply` |
//! | `packaging/keyboard/` | root helper, systemd unit, polkit policy |
//!
//! Flow: Settings sends the whole config to the helper (pkexec, no password
//! for the active user). The helper checks every line, writes the file and
//! starts the service. The service notices the new file within half a second
//! and reloads it in place.

pub mod config;
pub mod devices;
pub mod keys;
pub mod status;
