//! Startup apps: ~/.config/autostart entries and the launcher list.

use std::collections::HashMap;

use eframe::egui::Ui;

use super::*;
use crate::app::App;
use crate::icons::Icon;
use crate::pages::combo_row;
use crate::widgets::{self, Right};

#[derive(Clone)]
pub struct Autostart {
    pub file: String,
    pub name: String,
    pub enabled: bool,
}

pub(crate) fn dirs(paths: &[String], must_have: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for base in paths {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.join(must_have).exists() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !names.iter().any(|n| n == name) {
                    names.push(name.to_owned());
                }
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

pub(crate) fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

pub(crate) fn data_dirs() -> Vec<String> {
    let mut list = vec![std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.local/share", home()))];
    list.extend(
        std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned())
            .split(':')
            .map(str::to_owned),
    );
    list
}

pub(crate) fn autostart_dir() -> std::path::PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.config", home()));
    std::path::PathBuf::from(config).join("autostart")
}

pub(crate) fn desktop_field(text: &str, field: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some(v) = line.strip_prefix(field).and_then(|r| r.strip_prefix('=')) {
                return Some(v.trim().to_owned());
            }
        }
    }
    None
}

pub(crate) fn read_autostart() -> Vec<Autostart> {
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(autostart_dir()) else {
        return list;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = desktop_field(&text, "Name").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned()
        });
        let enabled = desktop_field(&text, "Hidden").as_deref() != Some("true")
            && desktop_field(&text, "X-GNOME-Autostart-enabled").as_deref() != Some("false");
        list.push(Autostart {
            file: path.to_string_lossy().into_owned(),
            name,
            enabled,
        });
    }
    list.sort_by_key(|a| a.name.to_lowercase());
    list
}

pub(crate) fn read_launchers() -> Vec<(String, String)> {
    let mut seen: HashMap<String, String> = HashMap::new();
    for base in data_dirs() {
        let Ok(entries) = std::fs::read_dir(format!("{base}/applications")) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if !id.ends_with(".desktop") || seen.contains_key(&id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if desktop_field(&text, "NoDisplay").as_deref() == Some("true")
                || desktop_field(&text, "Type").as_deref() != Some("Application")
            {
                continue;
            }
            if let Some(name) = desktop_field(&text, "Name") {
                seen.insert(id, path.to_string_lossy().into_owned() + "\u{1}" + &name);
            }
        }
    }
    let mut list: Vec<(String, String)> = seen
        .into_values()
        .filter_map(|v| {
            v.split_once('\u{1}')
                .map(|(p, n)| (p.to_owned(), n.to_owned()))
        })
        .collect();
    list.sort_by_key(|(_, n)| n.to_lowercase());
    list
}

pub(crate) fn startup_apps(app: &mut App, ui: &mut Ui, heading: bool) {
    let pal = app.pal();
    if heading {
        section_title(ui, app, "Startup apps");
    }
    let Some(loaded) = app.desk.loaded.as_ref() else {
        return;
    };
    let mut toggle: Option<(usize, bool)> = None;
    let mut remove: Option<usize> = None;
    let mut add: Option<String> = None;
    widgets::card(ui, &pal, |ui| {
        if loaded.autostart.is_empty() {
            widgets::empty_note(ui, &pal, "No apps start when you sign in.");
        }
        for (i, entry) in loaded.autostart.iter().enumerate() {
            if i > 0 {
                widgets::divider(ui, &pal);
            }
            let mut on = entry.enabled;
            ui.horizontal(|ui| {
                let row = ui.scope(|ui| {
                    ui.set_width(ui.available_width() - 90.0);
                    widgets::row(
                        ui,
                        &pal,
                        pal.accent,
                        Icon::Power,
                        &entry.name,
                        "Starts when you sign in",
                        Right::Switch(&mut on),
                    )
                });
                if row.inner.toggled() {
                    toggle = Some((i, on));
                }
                if widgets::ghost_button(ui, &pal, "Remove", None).clicked() {
                    remove = Some(i);
                }
            });
        }
        widgets::divider(ui, &pal);
        let opts: Vec<(String, String)> = loaded.launchers.clone();
        if let Some(path) = combo_row(
            ui,
            &pal,
            Icon::Apps,
            "Add a startup app",
            "Start it every time you sign in",
            "autostart-add",
            &opts,
            "",
            230.0,
        ) {
            add = Some(path);
        }
    });
    let dir = autostart_dir();
    let mut changed = false;
    if let Some((i, on)) = toggle {
        let file = loaded.autostart[i].file.clone();
        if let Ok(text) = std::fs::read_to_string(&file) {
            let mut out: Vec<String> = text
                .lines()
                .filter(|l| {
                    !l.starts_with("X-GNOME-Autostart-enabled=") && !l.starts_with("Hidden=")
                })
                .map(str::to_owned)
                .collect();
            let at = out
                .iter()
                .position(|l| l.trim() == "[Desktop Entry]")
                .map(|p| p + 1)
                .unwrap_or(out.len());
            out.insert(at, format!("X-GNOME-Autostart-enabled={on}"));
            changed = std::fs::write(&file, out.join("\n") + "\n").is_ok();
        }
    }
    if let Some(i) = remove {
        changed = std::fs::remove_file(&loaded.autostart[i].file).is_ok();
    }
    if let Some(path) = add {
        let _ = std::fs::create_dir_all(&dir);
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_owned());
        if let Some(name) = name {
            changed = std::fs::copy(&path, dir.join(name)).is_ok();
        }
    }
    if changed {
        if let Some(l) = app.desk.loaded.as_mut() {
            l.autostart = read_autostart();
        }
        app.notify("Startup apps updated");
    }
}
