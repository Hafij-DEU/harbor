//! Light/dark, accent, wallpaper, glass effect and Harbor's own saved design.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::*;
use crate::theme::Mode;

/// Set the wallpaper for light, dark and the lock screen from a local file.
pub fn set_wallpaper(path: &str) -> bool {
    let uri = if path.starts_with("file://") {
        path.to_owned()
    } else {
        format!("file://{}", path.replace(' ', "%20"))
    };
    let mut ok = true;
    for (schema, key) in [
        ("org.gnome.desktop.background", "picture-uri"),
        ("org.gnome.desktop.background", "picture-uri-dark"),
        ("org.gnome.desktop.screensaver", "picture-uri"),
    ] {
        ok = gsettings_set_string(schema, key, &uri) && ok;
    }
    ok
}

pub struct Appearance {
    pub mode: Mode,
    pub accent_name: String,
    pub accent: Option<(u8, u8, u8)>,
}

pub const GNOME_ACCENTS: [(&str, (u8, u8, u8)); 9] = [
    ("blue", (0x35, 0x84, 0xE4)),
    ("teal", (0x21, 0x90, 0xA4)),
    ("green", (0x3A, 0x94, 0x4A)),
    ("yellow", (0xC8, 0x88, 0x00)),
    ("orange", (0xED, 0x5B, 0x00)),
    ("red", (0xE6, 0x2D, 0x42)),
    ("pink", (0xD5, 0x61, 0x99)),
    ("purple", (0x91, 0x41, 0xAC)),
    ("slate", (0x6F, 0x83, 0x96)),
];

pub fn appearance() -> Appearance {
    let scheme = sh("gsettings get org.gnome.desktop.interface color-scheme")
        .trim_matches('\'')
        .to_owned();
    let theme = sh("gsettings get org.gnome.desktop.interface gtk-theme")
        .trim_matches('\'')
        .to_lowercase();
    let mode = if scheme == "prefer-dark" || (scheme.is_empty() && theme.contains("dark")) {
        Mode::Dark
    } else if scheme == "prefer-light" {
        Mode::Light
    } else if scheme.is_empty() {
        // No desktop bus at all: assume dark so the app still looks right.
        Mode::Dark
    } else {
        Mode::Light
    };

    let mut accent_name = sh("gsettings get org.gnome.desktop.interface accent-color")
        .trim_matches('\'')
        .to_owned();
    if accent_name.is_empty() {
        // Older desktops encode the accent in the theme name (Yaru-blue).
        accent_name = theme.rsplit('-').next().unwrap_or("").to_owned();
    }
    let accent = GNOME_ACCENTS
        .iter()
        .find(|(name, _)| *name == accent_name)
        .map(|(_, rgb)| *rgb);

    Appearance {
        mode,
        accent_name,
        accent,
    }
}

/// Where the chosen interface design is remembered between launches.
pub(crate) fn design_path() -> Option<std::path::PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("win11-settings").join("ui"))
}

/// `~/.config/harbor/<name>`: settings shared by every Harbor app.
pub(crate) fn harbor_path(name: &str) -> Option<PathBuf> {
    design_path().and_then(|p| p.parent()?.parent().map(|c| c.join("harbor").join(name)))
}

pub(crate) fn write_config(path: Option<PathBuf>, text: &str) {
    let Some(path) = path else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, text);
}

/// Where a glass style is set: the desktop (Fusion Desk, "global"), every
/// Harbor app, or one app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GlassScope {
    Global,
    Harbor,
    Settings,
    Files,
    TaskManager,
}

impl GlassScope {
    pub const ALL: [GlassScope; 5] = [
        GlassScope::Global,
        GlassScope::Harbor,
        GlassScope::Settings,
        GlassScope::Files,
        GlassScope::TaskManager,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GlassScope::Global => "Global",
            GlassScope::Harbor => "All Harbor apps",
            GlassScope::Settings => "Settings",
            GlassScope::Files => "Files",
            GlassScope::TaskManager => "Task Manager",
        }
    }

    /// The scope this one follows when it has no values of its own.
    pub fn parent(self) -> Option<GlassScope> {
        match self {
            GlassScope::Global => None,
            GlassScope::Harbor => Some(GlassScope::Global),
            _ => Some(GlassScope::Harbor),
        }
    }

    /// `~/.config/harbor/<file>`; Python apps read the same files.
    pub(crate) fn file(self) -> &'static str {
        match self {
            GlassScope::Global => "glass-global",
            GlassScope::Harbor => "glass",
            GlassScope::Settings => "glass-settings",
            GlassScope::Files => "glass-files",
            GlassScope::TaskManager => "glass-taskmgr",
        }
    }
}

/// A scope's own values; `None` follows its parent scope. All Harbor apps
/// start with glass off; each app starts following them.
pub fn scope_glass(scope: GlassScope) -> Option<crate::theme::Glass> {
    use crate::theme::Glass;
    if scope == GlassScope::Global {
        return Some(global_glass());
    }
    let mut text = harbor_path(scope.file()).and_then(|p| fs::read_to_string(p).ok());
    if text.is_none() && scope == GlassScope::Settings {
        // 0.5.18 previews kept this app's choice next to its design file.
        text = design_path().and_then(|p| fs::read_to_string(p.with_file_name("glass")).ok());
    }
    match text.as_deref().map(str::trim) {
        None if scope == GlassScope::Harbor => Some(Glass::OFF),
        None | Some("") | Some("global") | Some("harbor") => None,
        Some(t) => Some(Glass::from_text(t)),
    }
}

/// Save a scope's values, or make it follow its parent (`None`). The global
/// scope is Fusion Desk's own glass settings when it is installed.
pub fn save_scope_glass(scope: GlassScope, glass: Option<crate::theme::Glass>) -> bool {
    if scope == GlassScope::Global {
        let glass = glass.unwrap_or_default();
        write_config(harbor_path(scope.file()), &glass.to_text());
        return !crate::desktop::fusion_installed() || crate::desktop::set_fusion_glass(&glass);
    }
    let follow = if scope == GlassScope::Harbor {
        "global"
    } else {
        "harbor"
    };
    let text = glass
        .map(|g| g.to_text())
        .unwrap_or_else(|| follow.to_owned());
    write_config(harbor_path(scope.file()), &text);
    true
}

/// The desktop's glass: Fusion Desk's settings, or Harbor's copy of them
/// when Fusion Desk is not installed.
pub fn global_glass() -> crate::theme::Glass {
    crate::desktop::fusion_glass().unwrap_or_else(|| {
        harbor_path(GlassScope::Global.file())
            .and_then(|p| fs::read_to_string(p).ok())
            .map(|t| crate::theme::Glass::from_text(&t))
            .unwrap_or_default()
    })
}

/// The values a scope paints with, after following its parents.
pub fn resolve_glass(scope: GlassScope) -> crate::theme::Glass {
    let mut at = scope;
    loop {
        if let Some(g) = scope_glass(at) {
            return g;
        }
        match at.parent() {
            Some(p) => at = p,
            None => return crate::theme::Glass::OFF,
        }
    }
}

/// The glass this app paints with. `WIN11_SETTINGS_GLASS` (`off`, `glossy`,
/// `liquid` or `key=value,...`) overrides it for screenshot runs.
pub fn effective_glass() -> crate::theme::Glass {
    if let Ok(v) = std::env::var("WIN11_SETTINGS_GLASS") {
        return crate::theme::Glass::from_text(&v);
    }
    resolve_glass(GlassScope::Settings)
}

/// Whether the Settings window opens see-through. By default it is solid,
/// like Craft Suite, so GNOME draws its border and shadow and the cards
/// carry the glass. `WIN11_SETTINGS_SEE_THROUGH=1` opens it see-through
/// when the glass asks for it and Fusion Desk blurs what is behind.
pub fn glass_window() -> bool {
    std::env::var("WIN11_SETTINGS_SEE_THROUGH").is_ok_and(|v| v != "0")
        && effective_glass().see_through()
        && crate::desktop::fusion_blurs_windows()
}

/// Whether the Windows 10 home cards list their pages. On unless turned off;
/// `WIN11_SETTINGS_HOME_LINKS=0|1` overrides it for screenshot runs.
pub fn saved_home_links() -> bool {
    if let Ok(value) = std::env::var("WIN11_SETTINGS_HOME_LINKS") {
        return value != "0";
    }
    design_path()
        .and_then(|p| fs::read_to_string(p.with_file_name("home-links")).ok())
        .map(|text| text.trim() != "off")
        .unwrap_or(true)
}

pub fn save_home_links(on: bool) {
    if std::env::var("WIN11_SETTINGS_HOME_LINKS").is_ok() {
        return;
    }
    let Some(path) = design_path().map(|p| p.with_file_name("home-links")) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, if on { "on" } else { "off" });
}

/// Recently accessed pages, newest first, one `View::key` per line.
pub fn saved_recent() -> Vec<crate::app::View> {
    design_path()
        .and_then(|p| fs::read_to_string(p.with_file_name("recent")).ok())
        .map(|text| text.lines().filter_map(crate::app::View::from_key).collect())
        .unwrap_or_default()
}

pub fn save_recent(views: &[crate::app::View]) {
    if std::env::var("WIN11_SETTINGS_VIEW").is_ok() {
        return; // screenshot and test runs leave the list alone
    }
    let Some(path) = design_path().map(|p| p.with_file_name("recent")) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: String = views
        .iter()
        .filter_map(|v| v.key())
        .map(|k| k + "\n")
        .collect();
    let _ = fs::write(path, text);
}

/// Accent or Colourful icons. `WIN11_SETTINGS_LOOK=accent|colourful`
/// overrides it for screenshot runs.
pub fn saved_look() -> crate::theme::Look {
    if let Some(look) = std::env::var("WIN11_SETTINGS_LOOK")
        .ok()
        .and_then(|v| crate::theme::Look::from_key(&v))
    {
        return look;
    }
    design_path()
        .and_then(|p| fs::read_to_string(p.with_file_name("look")).ok())
        .and_then(|text| crate::theme::Look::from_key(&text))
        .unwrap_or_default()
}

pub fn save_look(look: crate::theme::Look) {
    if std::env::var("WIN11_SETTINGS_LOOK").is_ok() {
        return;
    }
    let Some(path) = design_path().map(|p| p.with_file_name("look")) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, look.key());
}

/// The interface design chosen in Personalisation. `WIN11_SETTINGS_UI`
/// overrides the saved choice (used by the screenshot and test runs).
pub fn saved_design() -> Option<crate::theme::Design> {
    if let Ok(value) = std::env::var("WIN11_SETTINGS_UI") {
        if let Some(design) = crate::theme::Design::from_key(&value) {
            return Some(design);
        }
    }
    let text = fs::read_to_string(design_path()?).ok()?;
    crate::theme::Design::from_key(&text)
}

/// Remember the interface design. The environment override is a test hook, so
/// it is deliberately not written back.
pub fn save_design(design: crate::theme::Design) {
    if std::env::var("WIN11_SETTINGS_UI").is_ok() {
        return;
    }
    let Some(path) = design_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, design.key());
}

pub fn set_color_scheme(mode: Mode) -> bool {
    let value = match mode {
        Mode::Dark => "prefer-dark",
        Mode::Light => "prefer-light",
    };
    Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", "color-scheme", value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Apply the closest GNOME accent to the desktop (Ubuntu 24.10+ key).
pub fn set_accent(r: u8, g: u8, b: u8) -> bool {
    if !command_exists("gsettings") {
        return false;
    }
    let name = GNOME_ACCENTS
        .iter()
        .min_by_key(|(_, rgb)| {
            let dr = rgb.0 as i32 - r as i32;
            let dg = rgb.1 as i32 - g as i32;
            let db = rgb.2 as i32 - b as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|(name, _)| *name)
        .unwrap_or("blue");
    Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", "accent-color", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
