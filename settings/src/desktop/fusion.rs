//! Fusion Desk shell extension: detection, install and its glass (blur) switch.

use std::process::Command;

use eframe::egui::{RichText, Ui};

use super::*;
use crate::app::App;
use crate::icons::Icon;
use crate::widgets::{self, Right, ROW_PAD};

pub(crate) const FUSION_SCHEMA: &str = "org.gnome.shell.extensions.fusion-desk";

pub(crate) const FUSION_UUID: &str = "fusion-desk@hafij-deu.github.io";

pub(crate) const FUSION_REPO: &str = "Hafij-DEU/Fusion-desk-gnome-extension";

/// Extension schemas are not in the system schema path: point gsettings at
/// the extension's own `schemas` folder.
/// Whether Fusion Desk's settings schema is installed.
pub fn fusion_installed() -> bool {
    !schema_args(FUSION_SCHEMA).is_empty()
}

/// Fusion Desk's glass and whether it has the Lumina keys (v49+), or
/// `None` when it is not installed.
pub fn fusion_glass() -> Option<(crate::theme::Glass, bool)> {
    if !fusion_installed() {
        return None;
    }
    let values = gsettings_dump(FUSION_SCHEMA);
    if values.is_empty() {
        return None;
    }
    let mut glass = crate::theme::Glass::default();
    for (key, value) in &values {
        if key.starts_with("glass-") && !glass.set(key, &plain(value)) {
            eprintln!("harbor-settings: Fusion Desk {key}={value} not understood");
        }
    }
    Some((glass, values.iter().any(|(k, _)| k == "glass-lumina")))
}

/// Whether Fusion Desk is running and blurs the desktop behind Harbor
/// windows. Without it a see-through window shows sharp windows behind it,
/// so Settings stays solid. `WIN11_SETTINGS_BLUR=0|1` overrides.
pub fn fusion_blurs_windows() -> bool {
    if let Ok(v) = std::env::var("WIN11_SETTINGS_BLUR") {
        return v != "0";
    }
    let on = gsettings_dump(FUSION_SCHEMA)
        .iter()
        .any(|(k, v)| k == "harbor-glass-blur" && v == "true");
    if !on {
        return false;
    }
    let get = |key: &str| {
        Command::new("gsettings")
            .args(["get", "org.gnome.shell", key])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    get("enabled-extensions").contains(FUSION_UUID)
        && !get("disabled-extensions").contains(FUSION_UUID)
        && get("disable-user-extensions").trim() != "true"
}

/// Write every glass setting to Fusion Desk: one `dconf load`, so the shell
/// restyles once; without the dconf tool, one `gsettings set` per key.
pub fn set_fusion_glass(glass: &crate::theme::Glass) -> bool {
    use crate::theme::Glass;
    let extra = schema_args(FUSION_SCHEMA);
    if extra.is_empty() {
        return false;
    }
    let keys = || Glass::KEYS.iter().chain(Glass::LUMINA_KEYS.iter());
    let batch: String = std::iter::once("[/]\n".to_owned())
        .chain(keys().map(|k| format!("{k}={}\n", glass.get(k))))
        .collect();
    if dconf_load("/org/gnome/shell/extensions/fusion-desk/", &batch) {
        return true;
    }
    let set = |key: &str| {
        let ok = Command::new("gsettings")
            .args(&extra)
            .args(["set", FUSION_SCHEMA, key, &glass.get(key)])
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("harbor-settings: gsettings set {key} failed");
        }
        ok
    };
    let ok = Glass::KEYS.iter().all(|key| set(key));
    // Fusion Desk before v49 has no Lumina settings: those may fail
    for key in Glass::LUMINA_KEYS {
        set(key);
    }
    ok
}

/// `dconf load <dir>` with `keyfile` on stdin; false when dconf is missing
/// or fails (the caller falls back to gsettings).
fn dconf_load(dir: &str, keyfile: &str) -> bool {
    use std::io::Write;
    let Ok(mut child) = Command::new("dconf")
        .args(["load", dir])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut s| s.write_all(keyfile.as_bytes()).is_ok());
    match child.wait_with_output() {
        Ok(out) if wrote && out.status.success() => true,
        Ok(out) => {
            eprintln!(
                "harbor-settings: dconf load failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            false
        }
        Err(_) => false,
    }
}

/// Fusion Desk status: install it when missing, turn it on or off, open its
/// own settings window.
pub(crate) fn fusion_header(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let ctx = ui.ctx().clone();
    poll(app, &ctx);
    if app.desk.extensions.is_none() && !app.desk.ext_loading {
        refresh_extensions(app, &ctx);
    }
    let ext = app
        .desk
        .extensions
        .as_ref()
        .and_then(|list| list.iter().find(|e| e.uuid == FUSION_UUID).cloned());
    let installing = app.desk.fusion_installing;
    let mut verb: Option<&'static str> = None;
    let mut install = false;
    widgets::card(ui, &pal, |ui| match &ext {
        None if app.desk.extensions.is_none() => {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.spinner();
                ui.label(RichText::new("Looking for Fusion Desk…").color(pal.text_muted));
            });
            ui.add_space(14.0);
        }
        None => {
            let sub = if installing {
                "Downloading and installing the latest release…"
            } else {
                "Taskbar, start menu, widgets and glass in one GNOME extension. Not installed."
            };
            widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Sparkle,
                "Fusion Desk",
                sub,
                Right::None,
            );
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD + 42.0);
                if installing {
                    ui.spinner();
                } else if widgets::button(
                    ui,
                    &pal,
                    pal.accent,
                    "Install Fusion Desk",
                    Some(Icon::Hub),
                )
                .clicked()
                {
                    install = true;
                }
            });
            ui.add_space(12.0);
        }
        Some(e) => {
            let mut on = e.enabled;
            let sub = format!(
                "Version {} · {}{}",
                e.version,
                state_text(e),
                if e.pending.is_empty() {
                    ""
                } else {
                    " · update applies after you sign in again"
                }
            );
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Sparkle,
                "Fusion Desk",
                &sub,
                Right::Switch(&mut on),
            )
            .toggled()
            {
                verb = Some(if on { "enable" } else { "disable" });
            }
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD + 42.0);
                if widgets::ghost_button(ui, &pal, "All Fusion Desk settings", None).clicked() {
                    verb = Some("prefs");
                }
            });
            ui.add_space(12.0);
        }
    });
    if let Some(verb) = verb {
        ext_action(app, &ctx, verb, FUSION_UUID.to_owned());
    }
    if install {
        app.desk.fusion_installing = true;
        spawn(app, &ctx, || Done::FusionInstalled(install_fusion()));
    }
}

/// Download the newest Fusion Desk release zip and install it for this user.
pub(crate) fn install_fusion() -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("win11-settings-fusion-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip = dir.join(format!("{FUSION_UUID}.shell-extension.zip"));
    let url = format!(
        "https://github.com/{FUSION_REPO}/releases/latest/download/{FUSION_UUID}.shell-extension.zip"
    );
    let zip_path = zip.to_string_lossy().into_owned();
    run(
        "curl",
        &["-fsSL", "--max-time", "300", "-o", &zip_path, &url],
    )
    .map_err(|e| format!("Download failed: {e}"))?;
    let res = run("gnome-extensions", &["install", "--force", &zip_path]);
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
    res.map_err(|e| format!("Install failed: {e}"))?;
    // The running shell only loads new extensions after a new login on
    // Wayland; enabling now makes it start then.
    let _ = run("gnome-extensions", &["enable", FUSION_UUID]);
    Ok("Fusion Desk installed. Sign out and back in to start it.".to_owned())
}
