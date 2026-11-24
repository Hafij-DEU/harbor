//! Time zone, clock format, NTP and system locale.

use std::fs;
use std::process::Command;

use super::*;

/// The IANA time-zone names known to this system (read once per process).
pub fn list_timezones() -> Vec<String> {
    static TIMEZONES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    TIMEZONES
        .get_or_init(|| {
            let out = sh("timedatectl list-timezones 2>/dev/null");
            if !out.is_empty() {
                return out
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            // Fall back to the zoneinfo tree.
            let mut zones = Vec::new();
            fn walk(dir: &std::path::Path, zones: &mut Vec<String>) {
                let Ok(entries) = fs::read_dir(dir) else {
                    return;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, zones);
                    } else if let Ok(rel) = path.strip_prefix("/usr/share/zoneinfo") {
                        let rel = rel.to_string_lossy().replace('\\', "/");
                        if !rel.is_empty()
                            && !rel.ends_with(".tab")
                            && !rel.ends_with(".list")
                            && !rel.starts_with("posix/")
                            && !rel.starts_with("right/")
                            && rel != "posixrules"
                        {
                            zones.push(rel);
                        }
                    }
                }
            }
            walk(std::path::Path::new("/usr/share/zoneinfo"), &mut zones);
            zones.sort();
            zones.dedup();
            zones
        })
        .clone()
}

/// Current local time in the same format the System probe uses.
pub fn local_time() -> String {
    sh("date '+%H:%M, %A %d %B %Y'")
}

/// Set the system time zone. Requires administrator authorisation via pkexec.
pub fn set_timezone(tz: &str) -> bool {
    if !tz
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '+'))
    {
        return false;
    }
    Command::new("pkexec")
        .args(["timedatectl", "set-timezone", tz])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_clock_24h(on: bool) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.interface",
            "clock-format",
            if on { "24h" } else { "12h" },
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_clock_detail(key: &str, on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.interface", key, on)
}

/// Toggle network time synchronisation.
pub fn set_ntp(on: bool) -> Result<(), String> {
    if !command_exists("pkexec") {
        return Err("pkexec is required to change the clock source".to_owned());
    }
    let value = if on { "true" } else { "false" };
    let out = sh(&format!("pkexec timedatectl set-ntp {value} 2>&1"));
    if out.contains("Error") || out.contains("error") {
        Err(clean_error(&out, "The clock source was not changed"))
    } else {
        Ok(())
    }
}

/// Every locale the system can generate, cached after the first read.
pub fn list_locales() -> Vec<String> {
    static CACHE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut locales: Vec<String> = sh("localectl list-locales 2>/dev/null")
                .lines()
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty() && l.contains('_'))
                .collect();
            locales.sort();
            locales.dedup();
            locales
        })
        .clone()
}

/// Set the system language for future sessions.
pub fn set_locale(locale: &str) -> Result<(), String> {
    if !command_exists("pkexec") {
        return Err("pkexec is required to change the language".to_owned());
    }
    let locale = locale.replace(['\'', ' ', '\n'], "");
    let script =
        format!("locale-gen '{locale}' >/dev/null 2>&1; localectl set-locale LANG='{locale}'");
    let out = sh(&format!("pkexec bash -c {} 2>&1", shell_quote(&script)));
    if out.contains("Error") || out.contains("error") {
        Err(clean_error(&out, "The language was not changed"))
    } else {
        Ok(())
    }
}
