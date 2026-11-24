//! Keyboard Manager, Settings side: talking to the root helper and service.
//! The helper is `packaging/keyboard/harbor-keyboard-apply`; polkit lets the
//! active user run it without a password. See `src/keyboard/mod.rs`.

use std::path::Path;
use std::process::{Command, Stdio};

use super::clean_error;

const HELPER: &str = "/usr/local/lib/win11-settings/harbor-keyboard-apply";
const SERVICE: &str = "harbor-keyboardd.service";
/// Toggle Internal Keyboard's own stuck-Shift watchdog.
const TOGGLE_WATCHDOG: &str = "toggle-internal-keyboard-watchdog.service";

/// Run the helper with `args`, feeding `stdin` to it.
fn helper(args: &[&str], stdin: &str, what: &str) -> Result<(), String> {
    if !Path::new(HELPER).exists() {
        return Err("Keyboard Manager is not installed; reinstall Harbor Settings".to_owned());
    }
    let mut child = Command::new("pkexec")
        .arg(HELPER)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut input) = child.stdin.take() {
        use std::io::Write;
        input
            .write_all(stdin.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(clean_error(&String::from_utf8_lossy(&out.stderr), what))
    }
}

/// Save the whole config (`Config::text`) and make sure the service runs.
pub fn save_keyboard_config(text: &str) -> Result<(), String> {
    helper(&["save"], text, "Could not save the keyboard settings")
}

/// Restart the service: keys disabled after sticking work again.
pub fn restart_keyboard_service() -> Result<(), String> {
    helper(&["restart"], "", "Could not restart the keyboard service")
}

/// Stop and disable Toggle Internal Keyboard's watchdog (the app itself stays).
pub fn stop_toggle_watchdog() -> Result<(), String> {
    helper(
        &["stop-toggle-watchdog"],
        "",
        "Could not turn the watchdog off",
    )
}

fn unit_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .is_ok_and(|s| s.success())
}

pub fn keyboard_service_running() -> bool {
    unit_active(SERVICE)
}

pub fn toggle_watchdog_running() -> bool {
    unit_active(TOGGLE_WATCHDOG)
}

/// Ask the service for Key check data (call at least every second while shown).
pub fn request_keycheck() {
    let uid = unsafe { libc::getuid() };
    let _ = std::fs::write(crate::keyboard::status::keycheck_request(uid), b"1");
}

/// Stop asking for Key check data.
pub fn stop_keycheck() {
    let uid = unsafe { libc::getuid() };
    let _ = std::fs::remove_file(crate::keyboard::status::keycheck_request(uid));
}
