//! Battery, power profiles, sleep, lid and power-button actions.

use std::fs;
use std::process::Command;

use super::*;

pub(crate) fn battery_state() -> (Option<u8>, bool) {
    let entries = match fs::read_dir("/sys/class/power_supply") {
        Ok(e) => e,
        Err(_) => return (None, false),
    };
    let mut pct = None;
    let mut charging = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("BAT") {
            continue;
        }
        let dir = entry.path();
        if let Ok(cap) = fs::read_to_string(dir.join("capacity")) {
            if let Ok(v) = cap.trim().parse::<u8>() {
                pct = Some(v);
            }
        }
        let status_path = dir.join("status");
        let status = read_trim(status_path.to_str().unwrap_or_default());
        if status == "Charging" || status == "Full" {
            charging = true;
        }
    }
    if pct.is_none() {
        let ac = fs::read_dir("/sys/class/power_supply")
            .ok()
            .map(|d| {
                d.flatten().any(|e| {
                    let online = e.path().join("online");
                    e.file_name().to_string_lossy().starts_with("AC")
                        && read_trim(online.to_str().unwrap_or_default()) == "1"
                })
            })
            .unwrap_or(false);
        charging = ac;
    }
    (pct, charging)
}

pub fn set_power_profile(profile: &str) -> bool {
    Command::new("powerprofilesctl")
        .args(["set", profile])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_idle_delay(seconds: u32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.session",
            "idle-delay",
            &format!("uint32 {}", seconds),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Suspend when idle on AC / battery. `true` = suspend, `false` = do nothing.
pub fn set_sleep_idle(ac: bool, battery: bool) -> bool {
    let ac_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "sleep-inactive-ac-type",
        if ac { "suspend" } else { "nothing" },
    );
    let battery_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "sleep-inactive-battery-type",
        if battery { "suspend" } else { "nothing" },
    );
    ac_ok && battery_ok
}

/// Lid close behaviour: pass "suspend" or "nothing" for each power source.
pub fn set_lid_action(ac: &str, battery: &str) -> bool {
    let ac_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "lid-close-ac-action",
        ac,
    );
    let battery_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "lid-close-battery-action",
        battery,
    );
    ac_ok && battery_ok
}

/// Power button behaviour: nothing | suspend | hibernate | interactive.
pub fn set_power_button(action: &str) -> bool {
    gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "power-button-action",
        action,
    )
}

pub fn set_low_battery_saver(on: bool) -> bool {
    gs_set_bool(
        "org.gnome.settings-daemon.plugins.power",
        "power-saver-profile-on-low-battery",
        on,
    )
}
