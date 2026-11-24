//! Monitors (Mutter D-Bus / xrandr), scale, brightness and night light.

use std::fs;
use std::process::Command;

use super::*;

/// Talks to the compositor's DisplayConfig API, which is what GNOME Settings
/// itself uses. On Wayland `xrandr` cannot change anything, so this is the only
/// way to make resolution, refresh rate and scale actually apply.
pub(crate) const MUTTER_SCRIPT: &str = r#"
import json, os, re, sys
from gi.repository import Gio, GLib

DEST = "org.gnome.Mutter.DisplayConfig"
PATH = "/org/gnome/Mutter/DisplayConfig"
IFACE = "org.gnome.Mutter.DisplayConfig"
BUS = Gio.bus_get_sync(Gio.BusType.SESSION, None)


def state():
    return BUS.call_sync(DEST, PATH, IFACE, "GetCurrentState", None, None,
                         Gio.DBusCallFlags.NONE, -1, None).unpack()


def xml_mode(connector):
    """Fallback current mode for compositors without the is-current flag."""
    try:
        text = open(os.path.expanduser("~/.config/monitors.xml")).read()
    except OSError:
        return None
    for block in text.split("<monitor>")[1:]:
        if "<connector>" + connector + "</connector>" not in block:
            continue
        def tag(name, block=block):
            m = re.search("<" + name + ">([0-9.]+)</" + name + ">", block)
            return float(m.group(1)) if m else None
        w, h, r = tag("width"), tag("height"), tag("rate")
        if w and h and r:
            return (int(w), int(h), r)
    return None


def describe(st):
    _serial, monitors, logical, _props = st
    scale_of = {}
    for (_x, _y, scale, _t, _p, mons, _mp) in logical:
        for (conn, _v, _p, _s) in mons:
            scale_of[conn] = scale
    out = []
    for (spec, modes, _mp) in monitors:
        conn = spec[0]
        fallback = xml_mode(conn)
        current = None
        for (mid, _w, _h, _r, _ps, _scales, mp) in modes:
            if mp.get("is-current"):
                current = mid
        if current is None and fallback is not None:
            for (mid, w, h, r, _ps, _scales, _mp) in modes:
                if (w, h) == fallback[:2] and abs(r - fallback[2]) < 0.1:
                    current = mid
                    break
        entries = []
        for (mid, w, h, rate, _ps, scales, _mp) in modes:
            entries.append({
                "id": mid, "width": w, "height": h, "rate": round(rate, 3),
                "scales": [round(sc, 3) for sc in scales],
                "current": mid == current,
            })
        entries.sort(key=lambda m: (m["width"] * m["height"], m["rate"]), reverse=True)
        out.append({
            "connector": conn,
            "scale": scale_of.get(conn, 1.0),
            "current": current or "",
            "modes": entries,
        })
    return out


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "no command"}))
        return
    cmd = sys.argv[1]
    try:
        st = state()
    except Exception as exc:  # noqa: BLE001 - surface the reason to the caller
        print(json.dumps({"error": str(exc)}))
        return
    if cmd == "list":
        print(json.dumps({"monitors": describe(st)}, separators=(",", ":")))
        return
    if cmd == "set":
        if len(sys.argv) < 4:
            print(json.dumps({"error": "usage"}))
            return
        connector, mode_id = sys.argv[2], sys.argv[3]
        scale = float(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[4] else None
        serial, monitors, logical, props = st
        current = {}
        for (spec, modes, _mp) in monitors:
            for (mid, _w, _h, _r, _ps, _scales, mp) in modes:
                if mp.get("is-current"):
                    current[spec[0]] = mid
        wanted = None
        for (spec, modes, _mp) in monitors:
            if spec[0] != connector:
                continue
            for (mid, _w, _h, _r, _ps, scales, _mp) in modes:
                if mid == mode_id:
                    wanted = (mid, [round(sc, 3) for sc in scales])
        if wanted is None:
            print(json.dumps({"error": "unknown mode"}))
            return
        if scale is not None and wanted[1] and round(scale, 3) not in wanted[1]:
            print(json.dumps({"error": "unsupported scale"}))
            return
        new_logical = []
        for (x, y, cur_scale, transform, primary, mons, _mp) in logical:
            new_mons = []
            for (conn, _v, _p, _s) in mons:
                new_mons.append((conn, wanted[0] if conn == connector else current.get(conn, ""), {}))
            new_scale = scale if (scale is not None and any(c == connector for (c, _v, _p, _s) in mons)) else cur_scale
            new_logical.append((x, y, new_scale, transform, primary, new_mons))
        gprops = {}
        if "layout-mode" in props:
            gprops["layout-mode"] = GLib.Variant("u", props["layout-mode"])
        BUS.call_sync(DEST, PATH, IFACE, "ApplyMonitorsConfig",
                      GLib.Variant("(uua(iiduba(ssa{sv}))a{sv})", (serial, 2, new_logical, gprops)),
                      None, Gio.DBusCallFlags.NONE, -1, None)
        print(json.dumps({"ok": True}))
        return
    print(json.dumps({"error": "unknown command"}))


main()
"#;

pub(crate) fn mutter_python(args: &[&str]) -> Option<String> {
    if !command_exists("python3") {
        return None;
    }
    let out = Command::new("python3")
        .arg("-c")
        .arg(MUTTER_SCRIPT)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Read the compositor's real mode list; falls back to parsing `xrandr` when
/// the DisplayConfig service is unavailable (X11 without Mutter).
pub(crate) fn display_monitors() -> Vec<DisplayMonitor> {
    if let Some(raw) = mutter_python(&["list"]) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            let mut monitors = Vec::new();
            for mon in value
                .get("monitors")
                .and_then(|m| m.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[])
            {
                let connector = mon
                    .get("connector")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                if connector.is_empty() {
                    continue;
                }
                let mut modes = Vec::new();
                for m in mon
                    .get("modes")
                    .and_then(|v| v.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[])
                {
                    modes.push(DisplayMode {
                        id: m
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_owned(),
                        width: m.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        height: m.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        rate: m.get("rate").and_then(|v| v.as_f64()).unwrap_or(60.0) as f32,
                        scales: m
                            .get("scales")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|s| s.as_f64())
                                    .map(|s| s as f32)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        current: m.get("current").and_then(|v| v.as_bool()).unwrap_or(false),
                    });
                }
                if modes.is_empty() {
                    continue;
                }
                monitors.push(DisplayMonitor {
                    connector,
                    scale: mon.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    modes,
                });
            }
            if !monitors.is_empty() {
                return monitors;
            }
        }
    }
    display_monitors_xrandr()
}

pub(crate) fn display_monitors_xrandr() -> Vec<DisplayMonitor> {
    let out = sh("xrandr --current 2>/dev/null");
    let mut monitors: Vec<DisplayMonitor> = Vec::new();
    for line in out.lines() {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let fields: Vec<&str> = line.split_whitespace().collect();
        if !indented {
            if fields.len() < 2 || !fields[1].starts_with("connected") {
                continue;
            }
            monitors.push(DisplayMonitor {
                connector: fields[0].to_owned(),
                scale: 1.0,
                modes: Vec::new(),
            });
            continue;
        }
        let Some(monitor) = monitors.last_mut() else {
            continue;
        };
        let Some(mode) = fields.first() else { continue };
        let (Some(w), Some(h)) = (
            mode.split('x').next().and_then(|v| v.parse::<u32>().ok()),
            mode.split('x').nth(1).and_then(|v| v.parse::<u32>().ok()),
        ) else {
            continue;
        };
        for token in &fields[1..] {
            let current = token.contains('*');
            let hz = token.trim_end_matches(['*', '+']);
            let Ok(rate) = hz.parse::<f32>() else {
                continue;
            };
            monitor.modes.push(DisplayMode {
                id: (*mode).to_owned(),
                width: w,
                height: h,
                rate,
                scales: vec![1.0],
                current,
            });
        }
    }
    monitors
}

/// Current resolution and refresh rate, taken from whichever display model
/// reports the active mode.
pub(crate) fn display_mode(monitors: &[DisplayMonitor]) -> (String, String) {
    for monitor in monitors {
        if let Some(mode) = monitor.modes.iter().find(|m| m.current) {
            return (
                format!("{} × {}", mode.width, mode.height),
                format!("{:.0} Hz", mode.rate),
            );
        }
    }
    ("Unknown".to_owned(), "—".to_owned())
}

/// Apply a mode (and optionally a fractional scale) through the compositor.
pub fn set_display_mode(connector: &str, mode_id: &str, scale: f32) -> Result<(), String> {
    let scale = format!("{scale}");
    match mutter_python(&["set", connector, mode_id, &scale]) {
        Some(raw) => {
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| format!("unexpected reply: {e}"))?;
            if value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                Ok(())
            } else {
                Err(value
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("the display server refused the change")
                    .to_owned())
            }
        }
        None => Err("display settings need python3-gi and a running desktop session".to_owned()),
    }
}

pub(crate) fn text_scale() -> String {
    let raw = gsettings_text("org.gnome.desktop.interface", "text-scaling-factor");
    let factor = raw.parse::<f32>().unwrap_or(1.0);
    format!("{:.0}%", factor * 100.0)
}

pub(crate) fn brightness() -> Option<u8> {
    let entries = fs::read_dir("/sys/class/backlight").ok()?;
    for entry in entries.flatten() {
        let dir = entry.path();
        let cur = fs::read_to_string(dir.join("brightness"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        let max = fs::read_to_string(dir.join("max_brightness"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        if max > 0.0 {
            return Some((cur / max * 100.0).round() as u8);
        }
    }
    None
}

pub fn set_night_light(on: bool) -> bool {
    gs_set_bool(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-enabled",
        on,
    )
}

/// Change the panel backlight through systemd-logind, which grants the active
/// session write access without root (the same path GNOME Settings uses).
pub(crate) fn logind_brightness(device: &str, value: u32) -> bool {
    let call = if command_exists("busctl") {
        sh(&format!(
            "busctl call org.freedesktop.login1 /org/freedesktop/login1/session/auto \
             org.freedesktop.login1.Session SetBrightness ssu backlight {} {} 2>/dev/null",
            shell_quote(device),
            value
        ))
    } else {
        sh(&format!(
            "gdbus call --system --dest org.freedesktop.login1 \
             --object-path /org/freedesktop/login1/session/auto \
             --method org.freedesktop.login1.Session.SetBrightness backlight {} {} 2>/dev/null",
            shell_quote(device),
            value
        ))
    };
    let _ = call;
    read_trim(&format!("/sys/class/backlight/{device}/brightness")) == value.to_string()
}

pub fn set_brightness(pct: u8) -> Result<(), String> {
    // logind first: it needs no password prompt and works for the active session.
    if let Ok(entries) = fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let device = entry.file_name().to_string_lossy().into_owned();
            let Ok(max) = fs::read_to_string(entry.path().join("max_brightness")) else {
                continue;
            };
            let Ok(max) = max.trim().parse::<f32>() else {
                continue;
            };
            let value = ((max * pct as f32 / 100.0).round() as u32).max(1);
            if logind_brightness(&device, value) {
                return Ok(());
            }
        }
    }
    // Prefer brightnessctl (works for users in the video group, no password prompt).
    if command_exists("brightnessctl") {
        let out = Command::new("brightnessctl")
            .args(["set", &format!("{}%", pct)])
            .output()
            .map_err(|e| e.to_string())?;
        return if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        };
    }
    // Fall back to writing sysfs directly; this only works when the node is writable.
    if let Ok(entries) = fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let dir = entry.path();
            if let (Ok(max), Ok(_cur)) = (
                fs::read_to_string(dir.join("max_brightness")),
                fs::read_to_string(dir.join("brightness")),
            ) {
                if let Ok(max) = max.trim().parse::<f32>() {
                    let value = (max * pct as f32 / 100.0).round() as u32;
                    if fs::write(dir.join("brightness"), value.to_string()).is_ok() {
                        return Ok(());
                    }
                }
            }
        }
    }
    Err("Backlight is not writable. Install brightnessctl and join the video group.".to_owned())
}

pub const NIGHT_LIGHT_DEFAULT_FROM: f32 = 20.0;

pub const NIGHT_LIGHT_DEFAULT_TO: f32 = 6.0;

/// A 00:00-24:00 window is how "no schedule" is represented in GSettings.
pub fn night_light_all_day(from: f32, to: f32) -> bool {
    from <= 0.01 && to >= 23.99
}

/// Turn the time window on or off. Off means the master switch applies at
/// once, which is what Windows does; GNOME otherwise keeps a manual window.
pub fn set_night_light_scheduled(on: bool, auto: bool, from: f32, to: f32) -> bool {
    if !on {
        return set_night_light_schedule(false, 0.0, 24.0);
    }
    if night_light_all_day(from, to) {
        return set_night_light_schedule(auto, NIGHT_LIGHT_DEFAULT_FROM, NIGHT_LIGHT_DEFAULT_TO);
    }
    set_night_light_schedule(auto, from, to)
}

pub fn set_night_light_schedule(auto: bool, from: f32, to: f32) -> bool {
    gs_set_bool(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-schedule-automatic",
        auto,
    ) && set_double(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-schedule-from",
        from,
    ) && set_double(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-schedule-to",
        to,
    )
}

pub fn set_night_light_temp(temp: u32) -> bool {
    gsettings_set_uint(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-temperature",
        temp,
    )
}
