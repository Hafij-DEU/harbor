//! Desktop entries, MIME defaults (mimeapps.list / xdg-mime) and launching.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::*;

// Desktop entries are scanned once per load (below) and shared by all categories.
thread_local! {
    /// Desktop entries (id, name, categories), scanned once per load so the
    /// three "default app" categories share a single directory walk.
    pub(crate) static DESKTOP_ENTRIES: std::cell::RefCell<Option<Vec<(String, String, String, String)>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn desktop_entries() -> Vec<(String, String, String, String)> {
    if let Some(hit) = DESKTOP_ENTRIES.with(|c| c.borrow().clone()) {
        return hit;
    }
    let mut roots = vec![
        "/usr/share/applications".to_owned(),
        "/var/lib/snapd/desktop/applications".to_owned(),
        "/var/lib/flatpak/exports/share/applications".to_owned(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(format!("{home}/.local/share/applications"));
        roots.push(format!(
            "{home}/.local/share/flatpak/exports/share/applications"
        ));
    }
    let mut found: Vec<(String, String, String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let desktop_id = entry.file_name().to_string_lossy().into_owned();
            if !seen.insert(desktop_id.clone()) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            // Read only the [Desktop Entry] group (skip actions such as "New Window").
            let mut in_entry = false;
            let mut no_display = false;
            let mut name: Option<String> = None;
            let mut categories = String::new();
            let mut mimes = String::new();
            for raw in text.lines() {
                let line = raw.trim();
                if line.starts_with('[') {
                    in_entry = line == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(v) = line.strip_prefix("NoDisplay=") {
                    no_display = v.trim().eq_ignore_ascii_case("true");
                } else if let Some(v) = line.strip_prefix("Name=") {
                    if name.is_none() {
                        name = Some(v.trim().to_owned());
                    }
                } else if let Some(v) = line.strip_prefix("Categories=") {
                    categories = v.to_owned();
                } else if let Some(v) = line.strip_prefix("MimeType=") {
                    mimes = v.to_owned();
                }
            }
            if no_display {
                continue;
            }
            let Some(name) = name else { continue };
            found.push((desktop_id, name, categories, mimes));
        }
    }
    DESKTOP_ENTRIES.with(|c| *c.borrow_mut() = Some(found.clone()));
    found
}

/// A file-type family whose default application can be changed in one go.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MimeDefault {
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub mimes: Vec<String>,
    pub current: String,
    pub mixed: bool,
    pub apps: Vec<(String, String)>,
}

pub(crate) struct MimeGroup {
    kind: &'static str,
    title: &'static str,
    subtitle: &'static str,
    mimes: &'static [&'static str],
}

pub(crate) const MIME_GROUPS: &[MimeGroup] = &[
    MimeGroup {
        kind: "document",
        title: "Documents",
        subtitle: "Word, OpenDocument and rich text",
        mimes: &[
            "application/msword",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.oasis.opendocument.text",
            "application/rtf",
        ],
    },
    MimeGroup {
        kind: "spreadsheet",
        title: "Spreadsheets",
        subtitle: "Excel, OpenDocument and CSV",
        mimes: &[
            "application/vnd.ms-excel",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.oasis.opendocument.spreadsheet",
            "text/csv",
        ],
    },
    MimeGroup {
        kind: "presentation",
        title: "Presentations",
        subtitle: "PowerPoint and OpenDocument slides",
        mimes: &[
            "application/vnd.ms-powerpoint",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "application/vnd.oasis.opendocument.presentation",
        ],
    },
    MimeGroup {
        kind: "pdf",
        title: "PDF documents",
        subtitle: "Open PDF files",
        mimes: &["application/pdf"],
    },
    MimeGroup {
        kind: "image",
        title: "Photos",
        subtitle: "PNG, JPEG, GIF and WebP",
        mimes: &["image/*"],
    },
    MimeGroup {
        kind: "video",
        title: "Video",
        subtitle: "MP4, MKV and WebM",
        mimes: &["video/*"],
    },
    MimeGroup {
        kind: "audio",
        title: "Music",
        subtitle: "MP3, FLAC, OGG and WAV",
        mimes: &["audio/*"],
    },
    MimeGroup {
        kind: "archive",
        title: "Archives",
        subtitle: "ZIP, 7z, RAR and tar",
        mimes: &[
            "application/zip",
            "application/x-7z-compressed",
            "application/vnd.rar",
            "application/x-tar",
            "application/gzip",
            "application/x-xz",
        ],
    },
    MimeGroup {
        kind: "calendar",
        title: "Calendar",
        subtitle: "ICS calendar files",
        mimes: &["text/calendar"],
    },
];

/// Does a desktop entry's MimeType list cover any of the wanted types?
pub(crate) fn declares_mime(declared: &str, mimes: &[String]) -> bool {
    declared
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .any(|d| {
            mimes.iter().any(|m| match m.strip_suffix("/*") {
                // "image/*" in a group matches "image/png", "image/jpeg", ...
                Some(prefix) => d.starts_with(&format!("{prefix}/")),
                None => m == d,
            })
        })
}

/// Build the default-application rows for the common file families.
pub(crate) fn mime_defaults() -> Vec<MimeDefault> {
    let entries = desktop_entries();
    MIME_GROUPS
        .iter()
        .map(|group| {
            let mimes: Vec<String> = group.mimes.iter().map(|m| (*m).to_owned()).collect();
            let mut apps: Vec<(String, String)> = entries
                .iter()
                .filter(|(_, _, _, declared)| declares_mime(declared, &mimes))
                .map(|(id, name, _, _)| (id.clone(), name.clone()))
                .collect();
            apps.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
            apps.dedup_by(|a, b| a.0 == b.0);

            let queried: Vec<String> = group.mimes.iter().map(|m| xdg_mime_query(m)).collect();
            let current = queried
                .iter()
                .find(|q| !q.is_empty())
                .cloned()
                .unwrap_or_default();
            let mixed = queried
                .iter()
                .filter(|q| !q.is_empty())
                .any(|q| *q != current);
            if !current.is_empty() && !apps.iter().any(|(id, _)| *id == current) {
                let name = entries
                    .iter()
                    .find(|(id, _, _, _)| *id == current)
                    .map(|(_, name, _, _)| name.clone())
                    .unwrap_or_else(|| current.clone());
                apps.insert(0, (current.clone(), name));
            }
            MimeDefault {
                kind: group.kind.to_owned(),
                title: group.title.to_owned(),
                subtitle: group.subtitle.to_owned(),
                mimes,
                current,
                mixed,
                apps,
            }
        })
        .collect()
}

pub(crate) fn handlers_with(category: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = desktop_entries()
        .into_iter()
        .filter(|(_, _, categories, _)| {
            categories
                .split(';')
                .any(|c| c.trim().eq_ignore_ascii_case(category))
        })
        .map(|(id, name, _, _)| (id, name))
        .collect();
    found.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    found
}

pub(crate) fn xdg_settings_get(key: &str) -> String {
    sh(&format!(
        "xdg-settings get {} 2>/dev/null",
        shell_quote(key)
    ))
}

thread_local! {
    /// MIME defaults parsed from the same files `xdg-mime` reads.
    pub(crate) static XDG_DEFAULTS: std::cell::RefCell<
        Option<std::collections::HashMap<String, String>>,
    > = const { std::cell::RefCell::new(None) };
}

/// Search path order that `xdg-mime` uses, so what we display is exactly what
/// `xdg-mime default` writes.
pub(crate) fn xdg_list(var: &str, fallback: &str) -> Vec<std::path::PathBuf> {
    let value = std::env::var(var)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback.to_owned());
    value
        .split(':')
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

pub(crate) fn xdg_data_dirs() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs = vec![std::path::PathBuf::from(format!("{home}/.local/share"))];
    dirs.extend(xdg_list("XDG_DATA_DIRS", "/usr/local/share:/usr/share"));
    dirs
}

pub(crate) fn xdg_config_dirs() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs = vec![std::path::PathBuf::from(
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("{home}/.config")),
    )];
    dirs.extend(xdg_list("XDG_CONFIG_DIRS", "/etc/xdg"));
    dirs
}

/// `mimeapps.list` variants: the plain file, then per-desktop overrides that
/// take precedence for the current desktop.
pub(crate) fn mimeapps_names() -> Vec<String> {
    let mut names = vec!["mimeapps.list".to_owned()];
    if let Ok(desktops) = std::env::var("XDG_CURRENT_DESKTOP") {
        for desktop in desktops.split(':').filter(|d| !d.is_empty()) {
            names.push(format!("{}-mimeapps.list", desktop.to_lowercase()));
        }
    }
    names
}

pub(crate) fn desktop_file_exists(id: &str) -> bool {
    if id.is_empty() || id == "-" || id.contains('/') {
        return false;
    }
    let mut roots: Vec<std::path::PathBuf> = xdg_data_dirs()
        .into_iter()
        .map(|d| d.join("applications"))
        .collect();
    if let Ok(home) = std::env::var("HOME") {
        roots.push(std::path::PathBuf::from(format!(
            "{home}/.local/share/applications"
        )));
    }
    roots.iter().any(|root| root.join(id).is_file())
}

pub(crate) fn parse_associations(
    path: &Path,
    defaults_only: bool,
    check_exists: bool,
    map: &mut std::collections::HashMap<String, String>,
) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let mut active = !defaults_only;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            active = !defaults_only || line == "[Default Applications]";
            continue;
        }
        if !active {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !key.contains('/') {
            continue;
        }
        let id = value
            .split(';')
            .map(str::trim)
            .find(|v| !v.is_empty() && (!check_exists || desktop_file_exists(v)));
        if let Some(id) = id {
            map.insert(key.to_owned(), id.to_owned());
        }
    }
}

/// Every MIME default, resolved the way `xdg-mime query default` does but
/// without spawning a process per type (that cost ~70 ms each).
pub(crate) fn xdg_defaults() -> std::collections::HashMap<String, String> {
    if let Some(hit) = XDG_DEFAULTS.with(|c| c.borrow().clone()) {
        return hit;
    }
    let mut map = std::collections::HashMap::new();
    let config = xdg_config_dirs();
    let data = xdg_data_dirs();
    let menu_prefix = std::env::var("XDG_MENU_PREFIX").unwrap_or_default();
    let names = mimeapps_names();

    // Lowest priority first so that later files overwrite earlier ones.
    for dir in data.iter().rev() {
        let apps = dir.join("applications");
        for name in ["mimeinfo.cache", "defaults.list"] {
            parse_associations(
                &apps.join(format!("{menu_prefix}{name}")),
                false,
                false,
                &mut map,
            );
            if !menu_prefix.is_empty() {
                parse_associations(&apps.join(name), false, false, &mut map);
            }
        }
    }
    for dir in data.iter().rev() {
        let apps = dir.join("applications");
        for name in &names {
            parse_associations(&apps.join(name), true, true, &mut map);
        }
    }
    for dir in config.iter().skip(1).collect::<Vec<_>>().into_iter().rev() {
        for name in &names {
            parse_associations(&dir.join(name), true, true, &mut map);
        }
    }
    for name in &names {
        parse_associations(&config[0].join(name), true, true, &mut map);
    }
    XDG_DEFAULTS.with(|c| *c.borrow_mut() = Some(map.clone()));
    map
}

pub(crate) fn xdg_default_for(mime: &str) -> String {
    if let Some(hit) = xdg_defaults().get(mime) {
        return hit.clone();
    }
    // Nothing configured: fall back to the first desktop entry that declares
    // the type, which is what xdg-mime's own fallback also ends up doing.
    let wanted = [mime.to_owned()];
    desktop_entries()
        .into_iter()
        .find(|(_, _, _, declared)| declares_mime(declared, &wanted))
        .map(|(id, _, _, _)| id)
        .unwrap_or_default()
}

pub(crate) fn xdg_mime_query(mime: &str) -> String {
    xdg_default_for(mime)
}

/// Every MIME type in the default-apps families and its resolved handler.
/// Used by the `WIN11_SETTINGS_MIME` diagnostic.
pub fn mime_report() -> Vec<(String, String)> {
    MIME_GROUPS
        .iter()
        .flat_map(|group| {
            group
                .mimes
                .iter()
                .map(|mime| ((*mime).to_owned(), xdg_default_for(mime)))
        })
        .collect()
}

/// Set the default web browser (desktop id, e.g. "firefox.desktop").
pub fn set_default_browser(desktop_id: &str) -> bool {
    Command::new("xdg-settings")
        .args(["set", "default-web-browser", desktop_id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Point every MIME type in a family at one application.
pub fn set_default_apps(mimes: &[String], desktop_id: &str) -> bool {
    let mut all = true;
    for mime in mimes {
        all &= set_default_for(mime, desktop_id);
    }
    all
}

/// Set the handler for a MIME type (desktop id).
pub fn set_default_for(mime: &str, desktop_id: &str) -> bool {
    XDG_DEFAULTS.with(|c| *c.borrow_mut() = None);
    Command::new("xdg-mime")
        .args(["default", desktop_id, mime])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Locate a themed PNG icon for a `.desktop` `Icon=` value.
pub fn find_icon(name: &str) -> Option<std::path::PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let direct = Path::new(name);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct.to_path_buf());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let mut roots = vec![
        "/usr/share/icons".to_owned(),
        "/var/lib/flatpak/exports/share/icons".to_owned(),
    ];
    if !home.is_empty() {
        roots.push(format!("{home}/.local/share/icons"));
        roots.push(format!("{home}/.local/share/flatpak/exports/share/icons"));
    }
    let themes = ["hicolor", "Yaru", "Adwaita", "breeze"];
    let sizes = ["256x256", "128x128", "96x96", "64x64", "48x48", "32x32"];
    for root in &roots {
        for theme in themes {
            for size in sizes {
                let candidate = Path::new(root)
                    .join(theme)
                    .join(size)
                    .join("apps")
                    .join(format!("{name}.png"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    for dir in ["/usr/share/pixmaps", "/var/lib/snapd/desktop/icons"] {
        let candidate = Path::new(dir).join(format!("{name}.png"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn open_path(path: &str) -> bool {
    command_exists("gio")
        && Command::new("gio")
            .args(["open", path])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
}

/// Launch a `.desktop` file by path (the App Hub inventory exposes these).
pub fn launch_desktop(path: &str) -> bool {
    if !command_exists("gio") {
        return false;
    }
    Command::new("gio")
        .arg("launch")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}
