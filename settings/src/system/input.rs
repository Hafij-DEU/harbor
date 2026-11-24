//! Keyboard layouts and repeat, mouse, touchpad and cursor settings.

use std::process::Command;

use super::*;

/// Parse `gsettings get org.gnome.desktop.input-sources sources`.
/// Returns the layout ids, e.g. ["us", "de"] (xkb sources only).
pub(crate) fn input_sources() -> Vec<String> {
    let raw = gsettings_text("org.gnome.desktop.input-sources", "sources");
    let mut ids = Vec::new();
    let mut rest = raw.as_str();
    while let Some(pos) = rest.find("('xkb'") {
        let after = rest[pos + "('xkb'".len()..].trim_start();
        let Some(after) = after.strip_prefix(',') else {
            break;
        };
        let Some(after) = after.trim_start().strip_prefix('\'') else {
            break;
        };
        let Some(end) = after.find('\'') else { break };
        let id = after[..end].trim();
        if !id.is_empty() {
            ids.push(id.to_owned());
        }
        rest = &after[end + 1..];
    }
    ids
}

/// Set the xkb input sources from layout ids (e.g. ["us", "de"]).
pub fn set_input_sources(ids: &[String]) -> bool {
    if ids.is_empty() {
        return false;
    }
    let tuple = ids
        .iter()
        .map(|id| format!("('xkb', '{}')", id.replace('\'', "")))
        .collect::<Vec<_>>()
        .join(", ");
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.input-sources",
            "sources",
            &format!("[{tuple}]"),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Common XKB layouts offered when adding an input source.
pub const KB_LAYOUTS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("br", "Portuguese (Brazil)"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("pl", "Polish"),
    ("nl", "Dutch"),
    ("be", "Belgian"),
    ("ch", "Swiss"),
    ("at", "Austrian"),
    ("se", "Swedish"),
    ("no", "Norwegian"),
    ("dk", "Danish"),
    ("fi", "Finnish"),
    ("cz", "Czech"),
    ("hu", "Hungarian"),
    ("ro", "Romanian"),
    ("gr", "Greek"),
    ("tr", "Turkish"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
    ("cn", "Chinese"),
    ("in", "India"),
    ("ara", "Arabic"),
    ("il", "Hebrew"),
    ("my", "Malay"),
    ("th", "Thai"),
    ("vn", "Vietnamese"),
    ("et", "Estonian"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("sk", "Slovak"),
    ("si", "Slovenian"),
    ("hr", "Croatian"),
    ("bg", "Bulgarian"),
    ("rs", "Serbian"),
];

pub(crate) fn input_state() -> (u32, bool) {
    if command_exists("pactl") {
        let vol = sh("pactl get-source-volume @DEFAULT_SOURCE@ 2>/dev/null");
        let volume = vol
            .split('%')
            .next()
            .and_then(|s| s.split_whitespace().last())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(70);
        let muted = sh("pactl get-source-mute @DEFAULT_SOURCE@ 2>/dev/null").contains("yes");
        return (volume, muted);
    }
    let out = sh("wpctl get-volume @DEFAULT_AUDIO_SOURCE@ 2>/dev/null");
    let volume = out
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v * 100.0).round() as u32)
        .unwrap_or(70);
    (volume, out.contains("[MUTED]"))
}

pub fn set_mouse_speed(value: f32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.peripherals.mouse",
            "speed",
            &format!("{:.3}", value.clamp(-1.0, 1.0)),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_mouse(flag: &str, on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.peripherals.mouse", flag, on)
}

pub fn set_touchpad(flag: &str, on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.peripherals.touchpad", flag, on)
}

pub fn set_touchpad_speed(value: f32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.peripherals.touchpad",
            "speed",
            &format!("{:.3}", value.clamp(-1.0, 1.0)),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_key_repeat(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.peripherals.keyboard", "repeat", on)
}

pub fn set_repeat_delay(ms: u32) -> bool {
    gsettings_set_uint("org.gnome.desktop.peripherals.keyboard", "delay", ms)
}

pub fn set_repeat_interval(ms: u32) -> bool {
    gsettings_set_uint(
        "org.gnome.desktop.peripherals.keyboard",
        "repeat-interval",
        ms,
    )
}

pub fn set_cursor_size(px: u32) -> bool {
    gsettings_set_uint("org.gnome.desktop.interface", "cursor-size", px)
}

pub(crate) fn set_double(schema: &str, key: &str, value: f32) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, &format!("{value}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
