//! Everything that talks to the system: probing for information and applying
//! settings through the standard Ubuntu tools (gsettings, pactl, nmcli, ...).

mod accessibility;
mod accounts;
mod appearance;
mod audio;
mod default_apps;
mod display;
mod gsettings;
mod info;
mod input;
mod keyboard;
mod network;
mod power;
mod privacy;
mod time_language;
mod updates;

use std::fs;
use std::process::{Command, Stdio};

pub use accessibility::*;
pub use accounts::*;
pub use appearance::*;
pub use audio::*;
pub use default_apps::*;
pub use display::*;
use gsettings::*;
pub use info::*;
pub use input::*;
pub use keyboard::*;
pub use network::*;
pub use power::*;
pub use privacy::*;
pub use time_language::*;
pub use updates::*;

pub fn sh(cmd: &str) -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        Err(_) => String::new(),
    }
}

pub fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", name))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn read_trim(path: &str) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn clean_error(raw: &str, fallback: &str) -> String {
    let line = raw
        .lines()
        .find(|l| l.contains("Error") || l.contains("error"))
        .unwrap_or("");
    if line.is_empty() {
        fallback.to_owned()
    } else {
        line.split(": ").last().unwrap_or(line).trim().to_owned()
    }
}

fn status_ok(
    status: std::io::Result<std::process::ExitStatus>,
    fallback: &str,
) -> Result<(), String> {
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(fallback.to_owned()),
        Err(e) => Err(format!("{fallback}: {e}")),
    }
}
