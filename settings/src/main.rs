mod app;
mod cleaner;
mod desktop;
mod hub;
mod icons;
mod inventory;
mod jobs;
mod jobview;
mod pages;
mod system;
mod theme;
mod welcome;
mod widgets;

use eframe::egui;

/// Process start, used by the `WIN11_SETTINGS_STARTUP` diagnostic.
pub static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn main() -> eframe::Result<()> {
    let _ = START.set(std::time::Instant::now());
    // `--ui win10` starts the same app in the Windows 10 interface without
    // changing the saved choice; a launcher entry uses this.
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if let Some(value) = arg.strip_prefix("--ui=") {
            Some(value.to_owned())
        } else if arg == "--ui" {
            args.next()
        } else {
            None
        };
        if let Some(value) = value {
            match value.as_str() {
                "win10" | "10" => std::env::set_var("WIN11_SETTINGS_UI", "win10"),
                "win11" | "11" => std::env::set_var("WIN11_SETTINGS_UI", "win11"),
                other => {
                    eprintln!("unknown interface '{other}' (use win10 or win11)");
                    return Ok(());
                }
            }
        }
    }
    // Diagnostic: list every pending update (apt, snap, Flatpak) and exit.
    if std::env::var("WIN11_SETTINGS_UPDATES").is_ok() {
        for u in system::check_updates() {
            println!("{:8} {:40} {} -> {}", u.source, u.name, u.old, u.new);
        }
        return Ok(());
    }
    // Diagnostic: time one full system probe without opening a window.
    if std::env::var("WIN11_SETTINGS_PROBE").is_ok() {
        let start = std::time::Instant::now();
        let info = system::Info::load();
        println!(
            "probe finished in {:.0} ms ({} modes on {} display(s))",
            start.elapsed().as_secs_f64() * 1000.0,
            info.display_monitors
                .iter()
                .map(|m| m.modes.len())
                .sum::<usize>(),
            info.display_monitors.len()
        );
        return Ok(());
    }
    // Diagnostic: run one discovery pass and report how long it takes.
    if std::env::var("WIN11_SETTINGS_SCAN").is_ok() {
        let start = std::time::Instant::now();
        match system::scan_bluetooth() {
            Ok(()) => println!(
                "bluetooth scan finished in {:.1} s",
                start.elapsed().as_secs_f64()
            ),
            Err(err) => println!(
                "bluetooth scan failed after {:.1} s: {err}",
                start.elapsed().as_secs_f64()
            ),
        }
        return Ok(());
    }
    // Diagnostic: the install recipes App Hub has saved.
    if std::env::var("WIN11_SETTINGS_FAVORITES").is_ok() {
        for favorite in hub::load_favorites() {
            println!(
                "{:<8} {:<30} installable={:<5} {}",
                favorite.channel_label(),
                favorite.name,
                favorite.installable(),
                favorite.describe()
            );
        }
        return Ok(());
    }
    // Diagnostic: how every default-application type resolves.
    if std::env::var("WIN11_SETTINGS_MIME").is_ok() {
        for (mime, id) in system::mime_report() {
            println!("{mime}\t{id}");
        }
        return Ok(());
    }
    // Diagnostic: read-only check of the pending upgrade list.
    if std::env::var("WIN11_SETTINGS_UPDATES").is_ok() {
        let updates = system::check_updates();
        println!(
            "{} pending update(s), phased={}",
            updates.len(),
            system::updates_phased()
        );
        for update in &updates {
            println!("  {} {} -> {}", update.name, update.old, update.new);
        }
        return Ok(());
    }
    // Diagnostic: exercise every setter, verify the write, then restore.
    if std::env::var("WIN11_SETTINGS_SELFTEST").is_ok() {
        let get = |schema: &str, key: &str| -> String {
            String::from_utf8_lossy(
                &std::process::Command::new("gsettings")
                    .args(["get", schema, key])
                    .output()
                    .map(|o| o.stdout)
                    .unwrap_or_default(),
            )
            .trim()
            .to_string()
        };
        let mut failures = 0;

        // Brightness: write the value that is already active, so nothing changes
        // on screen, but the logind path still has to succeed.
        let mut brightness = String::from("no backlight device");
        if let Ok(entries) = std::fs::read_dir("/sys/class/backlight") {
            for entry in entries.flatten() {
                let max = std::fs::read_to_string(entry.path().join("max_brightness"))
                    .ok()
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .unwrap_or(0.0);
                if max <= 0.0 {
                    continue;
                }
                let cur = std::fs::read_to_string(entry.path().join("brightness"))
                    .ok()
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .unwrap_or(0.0);
                let pct = ((cur / max * 100.0).round() as u8).clamp(1, 100);
                brightness = match system::set_brightness(pct) {
                    Ok(()) => {
                        let now = std::fs::read_to_string(entry.path().join("brightness"))
                            .ok()
                            .and_then(|v| v.trim().parse::<f32>().ok())
                            .unwrap_or(-1.0);
                        let shown = (now / max * 100.0).round();
                        format!("ok ({}% -> {:.0}%)", pct, shown)
                    }
                    Err(e) => {
                        failures += 1;
                        format!("FAILED ({e})")
                    }
                };
                break;
            }
        }

        let toggle = |schema: &str,
                      key: &str,
                      setter: &dyn Fn(bool) -> bool,
                      failures: &mut u32|
         -> String {
            let before = get(schema, key);
            let target = before != "true";
            let ok = setter(target);
            let after = get(schema, key);
            let changed = after == if target { "true" } else { "false" };
            let restored = if setter(before == "true") {
                get(schema, key) == before
            } else {
                false
            };
            if ok && changed && restored {
                format!("ok ({before} -> {after} -> {before})")
            } else {
                *failures += 1;
                format!(
                    "FAILED (ok={ok} changed={changed} restored={restored}, now {})",
                    get(schema, key)
                )
            }
        };

        println!("brightness           {}", brightness);
        println!(
            "night light          {}",
            toggle(
                "org.gnome.settings-daemon.plugins.color",
                "night-light-enabled",
                &system::set_night_light,
                &mut failures
            )
        );
        println!(
            "mouse natural scroll {}",
            toggle(
                "org.gnome.desktop.peripherals.mouse",
                "natural-scroll",
                &|on| system::set_mouse("natural-scroll", on),
                &mut failures
            )
        );
        println!(
            "touchpad tap-to-click {}",
            toggle(
                "org.gnome.desktop.peripherals.touchpad",
                "tap-to-click",
                &|on| system::set_touchpad("tap-to-click", on),
                &mut failures
            )
        );
        println!(
            "touchpad two-finger   {}",
            toggle(
                "org.gnome.desktop.peripherals.touchpad",
                "two-finger-scrolling-enabled",
                &|on| system::set_touchpad("two-finger-scrolling-enabled", on),
                &mut failures
            )
        );
        println!(
            "keyboard repeat       {}",
            toggle(
                "org.gnome.desktop.peripherals.keyboard",
                "repeat",
                &system::set_key_repeat,
                &mut failures
            )
        );
        // Night light: with no schedule it must actually become active (GNOME
        // otherwise waits for the 20:00-06:00 window).
        let nl_was_on = get(
            "org.gnome.settings-daemon.plugins.color",
            "night-light-enabled",
        ) == "true";
        let schedule_ok = system::set_night_light_scheduled(false, false, 0.0, 24.0)
            && system::set_night_light(true);
        std::thread::sleep(std::time::Duration::from_millis(2200));
        let active = String::from_utf8_lossy(
            &std::process::Command::new("busctl")
                .args([
                    "--user",
                    "get-property",
                    "org.gnome.SettingsDaemon.Color",
                    "/org/gnome/SettingsDaemon/Color",
                    "org.gnome.SettingsDaemon.Color",
                    "NightLightActive",
                ])
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default(),
        )
        .contains("true");
        let restored = system::set_night_light(nl_was_on);
        if !(schedule_ok && active && restored) {
            failures += 1;
        }
        println!(
            "night light active   {}",
            if schedule_ok && active && restored {
                "ok (switch warms immediately)"
            } else {
                "FAILED (schedule={schedule_ok} active={active} restored={restored})"
            }
        );

        // Pointer size.
        let cursor_before = get("org.gnome.desktop.interface", "cursor-size");
        let cursor_ok = system::set_cursor_size(32)
            && get("org.gnome.desktop.interface", "cursor-size")
                .split_whitespace()
                .last()
                == Some("32")
            && system::set_cursor_size(
                cursor_before
                    .split_whitespace()
                    .last()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(24),
            );
        if !cursor_ok {
            failures += 1;
        }
        println!(
            "pointer size         {}",
            if cursor_ok {
                "ok (32 px written, previous restored)"
            } else {
                "FAILED"
            }
        );

        // Accessibility and privacy switches (all were dead before the
        // gs_set_bool fix, so verify each one writes and restores).
        let check = |name: &str,
                     schema: &str,
                     key: &str,
                     setter: &dyn Fn(bool) -> bool,
                     failures: &mut u32| {
            let before = get(schema, key);
            let target = before != "true";
            let ok = setter(target);
            let changed = get(schema, key) == if target { "true" } else { "false" };
            let restored = setter(before == "true") && get(schema, key) == before;
            if ok && changed && restored {
                println!("{name:<21}ok");
            } else {
                *failures += 1;
                println!("{name:<21}FAILED (ok={ok} changed={changed} restored={restored})");
            }
        };
        check(
            "visual alerts",
            "org.gnome.desktop.wm.preferences",
            "visual-bell",
            &system::set_visual_alerts,
            &mut failures,
        );
        check(
            "sticky keys",
            "org.gnome.desktop.a11y.keyboard",
            "stickykeys-enable",
            &system::set_sticky_keys,
            &mut failures,
        );
        check(
            "mouse keys",
            "org.gnome.desktop.a11y.keyboard",
            "mousekeys-enable",
            &system::set_mouse_keys,
            &mut failures,
        );
        check(
            "animations",
            "org.gnome.desktop.interface",
            "enable-animations",
            &system::set_animations,
            &mut failures,
        );
        check(
            "screen lock",
            "org.gnome.desktop.screensaver",
            "lock-enabled",
            &system::set_screen_lock,
            &mut failures,
        );
        check(
            "location",
            "org.gnome.system.location",
            "enabled",
            &system::set_location,
            &mut failures,
        );
        check(
            "camera block",
            "org.gnome.desktop.privacy",
            "disable-camera",
            &|on| system::set_camera(!on),
            &mut failures,
        );
        check(
            "microphone block",
            "org.gnome.desktop.privacy",
            "disable-microphone",
            &|on| system::set_microphone(!on),
            &mut failures,
        );
        check(
            "recent files",
            "org.gnome.desktop.privacy",
            "remember-recent-files",
            &system::set_remember_recent,
            &mut failures,
        );
        check(
            "old temp files",
            "org.gnome.desktop.privacy",
            "remove-old-temp-files",
            &system::set_remove_temp,
            &mut failures,
        );
        check(
            "problem reports",
            "org.gnome.desktop.privacy",
            "report-technical-problems",
            &system::set_report_problems,
            &mut failures,
        );
        check(
            "show date",
            "org.gnome.desktop.interface",
            "clock-show-date",
            &|on| system::set_clock_detail("clock-show-date", on),
            &mut failures,
        );
        check(
            "show weekday",
            "org.gnome.desktop.interface",
            "clock-show-weekday",
            &|on| system::set_clock_detail("clock-show-weekday", on),
            &mut failures,
        );
        check(
            "show seconds",
            "org.gnome.desktop.interface",
            "clock-show-seconds",
            &|on| system::set_clock_detail("clock-show-seconds", on),
            &mut failures,
        );

        // 24-hour clock is an enum, not a boolean.
        let format_before = get("org.gnome.desktop.interface", "clock-format");
        let was_24 = format_before.trim_matches('\'') == "24h";
        let want_24 = !was_24;
        let format_ok = system::set_clock_24h(want_24)
            && get("org.gnome.desktop.interface", "clock-format").trim_matches('\'')
                == if want_24 { "24h" } else { "12h" }
            && system::set_clock_24h(was_24);
        if !format_ok {
            failures += 1;
        }
        println!(
            "24-hour clock        {}",
            if format_ok { "ok" } else { "FAILED" }
        );

        // Text scaling.
        let scale_before = get("org.gnome.desktop.interface", "text-scaling-factor");
        let scale_ok = system::set_text_scale(1.25)
            && get("org.gnome.desktop.interface", "text-scaling-factor") == "1.25"
            && system::set_text_scale(scale_before.parse::<f32>().unwrap_or(1.0));
        if !scale_ok {
            failures += 1;
        }
        println!(
            "text scaling         {}",
            if scale_ok { "ok" } else { "FAILED" }
        );

        // Input sources: add a second layout through the same setter the UI uses.
        let sources_before = String::from_utf8_lossy(
            &std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.input-sources", "sources"])
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default(),
        )
        .trim()
        .to_string();
        let add_ok = system::set_input_sources(&["us".to_owned(), "de".to_owned()]);
        let added = String::from_utf8_lossy(
            &std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.input-sources", "sources"])
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default(),
        )
        .contains("de");
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.input-sources",
                "sources",
                &sources_before,
            ])
            .status();
        if !(add_ok && added) {
            failures += 1;
        }
        println!(
            "keyboard layouts     {}",
            if add_ok && added {
                "ok (us+de added, previous restored)"
            } else {
                "FAILED"
            }
        );

        // Default applications: retarget PDF, check xdg-mime, then put it back.
        let info = system::Info::load();
        for group in &info.mime_defaults {
            println!(
                "  default {:<14} {:>3} apps, current {}",
                group.title,
                group.apps.len(),
                if group.mixed {
                    "mixed".to_owned()
                } else if group.current.is_empty() {
                    "none".to_owned()
                } else {
                    group.current.clone()
                }
            );
        }
        let pdf = info
            .mime_defaults
            .iter()
            .find(|group| group.kind == "pdf")
            .cloned();
        let mut pdf_result = String::from("no pdf group");
        if let Some(group) = pdf {
            let before = String::from_utf8_lossy(
                &std::process::Command::new("xdg-mime")
                    .args(["query", "default", "application/pdf"])
                    .output()
                    .map(|o| o.stdout)
                    .unwrap_or_default(),
            )
            .trim()
            .to_string();
            if let Some((id, name)) = group.apps.iter().find(|(id, _)| *id != before) {
                let set_ok = system::set_default_apps(&group.mimes, id);
                let now = String::from_utf8_lossy(
                    &std::process::Command::new("xdg-mime")
                        .args(["query", "default", "application/pdf"])
                        .output()
                        .map(|o| o.stdout)
                        .unwrap_or_default(),
                )
                .trim()
                .to_string();
                let restored =
                    before.is_empty() || system::set_default_for("application/pdf", &before);
                if set_ok && now == *id && restored {
                    let from = if before.is_empty() {
                        "none"
                    } else {
                        before.as_str()
                    };
                    pdf_result = format!("ok ({from} -> {name}, restored)");
                } else {
                    failures += 1;
                    pdf_result = format!("FAILED (set={set_ok} now={now})");
                }
            } else {
                pdf_result = "skipped (no alternative PDF app)".to_owned();
            }
        }
        println!("default applications {pdf_result}");

        println!("failures: {failures}");
        std::process::exit(if failures == 0 { 0 } else { 1 });
    }
    let env_f32 = |name: &str, default: f32| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(default)
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Settings")
            .with_inner_size([
                env_f32("WIN11_SETTINGS_WIDTH", 1280.0),
                env_f32("WIN11_SETTINGS_HEIGHT", 800.0),
            ])
            .with_min_inner_size([1024.0, 680.0])
            .with_decorations(false)
            .with_resizable(true)
            .with_app_id("win11-settings")
            .with_transparent(system::effective_glass().see_through()),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Settings",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
