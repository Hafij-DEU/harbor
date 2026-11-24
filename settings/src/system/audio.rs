//! Output and input devices through PipeWire (wpctl) and PulseAudio (pactl).

use std::path::Path;
use std::process::Command;

use super::*;

pub(crate) fn audio_state() -> (u32, bool) {
    if command_exists("pactl") {
        let vol = sh("pactl get-sink-volume @DEFAULT_SINK@ 2>/dev/null");
        let volume = vol
            .split('%')
            .next()
            .and_then(|s| s.split_whitespace().last())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(50);
        let muted = sh("pactl get-sink-mute @DEFAULT_SINK@ 2>/dev/null").contains("yes");
        return (volume, muted);
    }
    // PipeWire's own control tool. "Volume: 0.65 [MUTED]"
    let out = sh("wpctl get-volume @DEFAULT_AUDIO_SINK@ 2>/dev/null");
    let volume = out
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v * 100.0).round() as u32)
        .unwrap_or(50);
    let muted = out.contains("[MUTED]");
    (volume, muted)
}

pub(crate) fn default_sink_name() -> String {
    if command_exists("pactl") {
        let name = sh("pactl get-default-sink");
        if !name.is_empty() {
            return name;
        }
    }
    wpctl_property("@DEFAULT_AUDIO_SINK@", "node.name")
}

pub(crate) fn wpctl_property(target: &str, key: &str) -> String {
    let out = sh(&format!(
        "wpctl inspect {} 2>/dev/null",
        shell_quote(target)
    ));
    out.lines()
        .find_map(|line| line.split_once('=').filter(|(k, _)| k.contains(key)))
        .map(|(_, v)| v.trim().trim_matches('"').to_owned())
        .unwrap_or_default()
}

pub(crate) fn sinks() -> Vec<Sink> {
    if command_exists("pactl") {
        let out = sh("pactl list sinks 2>/dev/null");
        let mut sinks = Vec::new();
        let mut name = String::new();
        let mut desc = String::new();
        for line in out.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Name: ") {
                name = rest.to_owned();
                desc.clear();
            } else if let Some(rest) = line.strip_prefix("Description: ") {
                desc = rest.to_owned();
                if !name.is_empty() {
                    sinks.push(Sink {
                        name: name.clone(),
                        description: desc.clone(),
                        id: String::new(),
                    });
                }
            }
        }
        if !sinks.is_empty() {
            return sinks;
        }
    }
    wpctl_sinks()
}

/// Parse the "Sinks:" block of `wpctl status`.
pub(crate) fn wpctl_sinks() -> Vec<Sink> {
    let out = sh("wpctl status 2>/dev/null");
    let mut sinks = Vec::new();
    let mut in_sinks = false;
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("Sinks:") {
            in_sinks = true;
            continue;
        }
        if !in_sinks {
            continue;
        }
        let rest = trimmed
            .rsplit('│')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .trim_start_matches('*')
            .trim();
        if rest.is_empty() {
            continue;
        }
        let Some((id, tail)) = rest.split_once('.') else {
            break;
        };
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            // End of the Sinks block (Sources:, Filters:, ...).
            break;
        }
        let description = tail.split('[').next().unwrap_or(tail).trim().to_owned();
        let name = wpctl_property(id, "node.name");
        sinks.push(Sink {
            name: if name.is_empty() { id.to_owned() } else { name },
            description: if description.is_empty() {
                format!("Output {id}")
            } else {
                description
            },
            id: id.to_owned(),
        });
    }
    sinks
}

pub(crate) fn default_source_name() -> String {
    if command_exists("pactl") {
        let name = sh("pactl get-default-source");
        if !name.is_empty() {
            return name;
        }
    }
    wpctl_property("@DEFAULT_AUDIO_SOURCE@", "node.name")
}

pub(crate) fn sources() -> Vec<Sink> {
    if command_exists("pactl") {
        let out = sh("pactl list sources 2>/dev/null");
        let mut sinks = Vec::new();
        let mut name = String::new();
        let mut desc = String::new();
        for line in out.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Name: ") {
                name = rest.to_owned();
                desc.clear();
            } else if let Some(rest) = line.strip_prefix("Description: ") {
                desc = rest.to_owned();
                // Skip monitor sources — they are not real microphones.
                if !name.is_empty() && !name.ends_with(".monitor") {
                    sinks.push(Sink {
                        name: name.clone(),
                        description: desc.clone(),
                        id: String::new(),
                    });
                }
            }
        }
        if !sinks.is_empty() {
            return sinks;
        }
    }
    wpctl_sources()
}

/// Parse the "Sources:" block of `wpctl status`.
pub(crate) fn wpctl_sources() -> Vec<Sink> {
    let out = sh("wpctl status 2>/dev/null");
    let mut sinks = Vec::new();
    let mut in_sources = false;
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("Sources:") {
            in_sources = true;
            continue;
        }
        if !in_sources {
            continue;
        }
        let rest = trimmed
            .rsplit('│')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .trim_start_matches('*')
            .trim();
        if rest.is_empty() {
            continue;
        }
        let Some((id, tail)) = rest.split_once('.') else {
            break;
        };
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        let description = tail.split('[').next().unwrap_or(tail).trim().to_owned();
        let name = wpctl_property(id, "node.name");
        sinks.push(Sink {
            name: if name.is_empty() { id.to_owned() } else { name },
            description: if description.is_empty() {
                format!("Input {id}")
            } else {
                description
            },
            id: id.to_owned(),
        });
    }
    sinks
}

pub fn set_volume(pct: u32) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", pct)])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-volume",
            "@DEFAULT_AUDIO_SINK@",
            &format!("{:.2}", pct as f32 / 100.0),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_muted(muted: bool) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args([
                "set-sink-mute",
                "@DEFAULT_SINK@",
                if muted { "1" } else { "0" },
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-mute",
            "@DEFAULT_AUDIO_SINK@",
            if muted { "1" } else { "0" },
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_input_volume(pct: u32) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args([
                "set-source-volume",
                "@DEFAULT_SOURCE@",
                &format!("{}%", pct),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-volume",
            "@DEFAULT_AUDIO_SOURCE@",
            &format!("{:.2}", pct as f32 / 100.0),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_input_muted(muted: bool) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args([
                "set-source-mute",
                "@DEFAULT_SOURCE@",
                if muted { "1" } else { "0" },
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-mute",
            "@DEFAULT_AUDIO_SOURCE@",
            if muted { "1" } else { "0" },
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_default_source(name: &str) -> bool {
    if command_exists("pactl") {
        let ok = Command::new("pactl")
            .args(["set-default-source", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    let id = sources()
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .unwrap_or_else(|| name.to_owned());
    if id.is_empty() {
        return false;
    }
    Command::new("wpctl")
        .args(["set-default", &id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_default_sink(name: &str) -> bool {
    if command_exists("pactl") {
        let ok = Command::new("pactl")
            .args(["set-default-sink", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    // wpctl wants a numeric id: find the sink whose node.name matches.
    let id = sinks()
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .unwrap_or_else(|| name.to_owned());
    if id.is_empty() {
        return false;
    }
    Command::new("wpctl")
        .args(["set-default", &id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn audio_available() -> bool {
    command_exists("pactl") || command_exists("wpctl")
}

pub fn test_sound() {
    let candidates = [
        "/usr/share/sounds/freedesktop/stereo/complete.oga",
        "/usr/share/sounds/gnome/default/alerts/bell.oga",
        "/usr/share/sounds/ubuntu/stereo/bell.ogg",
    ];
    let players: [(&str, bool); 3] = [
        ("paplay", false),
        ("pw-play", false),
        ("canberra-gtk-play", true),
    ];
    for path in candidates {
        if !Path::new(path).exists() {
            continue;
        }
        for (player, use_file_flag) in players {
            if !command_exists(player) {
                continue;
            }
            let mut cmd = Command::new(player);
            if use_file_flag {
                cmd.arg("-f");
            }
            if cmd.arg(path).spawn().is_ok() {
                return;
            }
        }
    }
}
