//! Reading every catalogued key in one gsettings dump and writing single keys.

use std::collections::HashMap;
use std::process::Command;

use super::*;

#[derive(Clone)]
pub struct Found {
    pub uuid: String,
    pub name: String,
    pub description: String,
}

pub(crate) struct Loaded {
    pub(crate) values: HashMap<String, String>,
    pub(crate) fonts: Vec<String>,
    pub(crate) themes: HashMap<u8, Vec<String>>,
    pub(crate) autostart: Vec<Autostart>,
    pub(crate) launchers: Vec<(String, String)>,
}

pub(crate) fn schema_args(schema: &str) -> Vec<String> {
    if schema != FUSION_SCHEMA {
        return Vec::new();
    }
    let candidates = [
        format!(
            "{}/.local/share/gnome-shell/extensions/{FUSION_UUID}/schemas",
            home()
        ),
        format!("/usr/share/gnome-shell/extensions/{FUSION_UUID}/schemas"),
        format!("/usr/local/share/gnome-shell/extensions/{FUSION_UUID}/schemas"),
    ];
    candidates
        .into_iter()
        .find(|d| std::path::Path::new(d).join("gschemas.compiled").exists())
        .map(|d| vec!["--schemadir".to_owned(), d])
        .unwrap_or_default()
}

pub(crate) fn gsettings_dump(schema: &str) -> Vec<(String, String)> {
    let extra = schema_args(schema);
    if schema == FUSION_SCHEMA && extra.is_empty() {
        return Vec::new();
    }
    let out = Command::new("gsettings")
        .args(&extra)
        .args(["list-recursively", schema])
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut it = line.splitn(3, ' ');
            let (_, key, value) = (it.next()?, it.next()?, it.next()?);
            Some((key.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// GVariant text to a plain string: 'Bottom' -> Bottom, uint32 48 -> 48.
pub(crate) fn plain(value: &str) -> String {
    let v = value.trim();
    let v = v
        .strip_prefix("uint32 ")
        .or_else(|| v.strip_prefix("int32 "))
        .unwrap_or(v);
    v.trim_matches('\'').to_owned()
}

pub(crate) fn load() -> Loaded {
    let mut values = HashMap::new();
    let mut schemas: Vec<&str> = ITEMS.iter().map(|i| i.schema).collect();
    schemas.sort();
    schemas.dedup();
    for schema in schemas {
        for (key, value) in gsettings_dump(schema) {
            values.insert(format!("{schema} {key}"), value);
        }
    }
    let fonts = Command::new("fc-list")
        .args([":", "family"])
        .output()
        .map(|o| {
            let mut list: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split(',').next())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty() && !s.starts_with('.'))
                .collect();
            list.sort_by_key(|s| s.to_lowercase());
            list.dedup();
            list
        })
        .unwrap_or_default();
    let data = data_dirs();
    let mut themes = HashMap::new();
    let theme_dirs: Vec<String> = data
        .iter()
        .map(|d| format!("{d}/themes"))
        .chain(std::iter::once(format!("{}/.themes", home())))
        .collect();
    let icon_dirs: Vec<String> = data
        .iter()
        .map(|d| format!("{d}/icons"))
        .chain(std::iter::once(format!("{}/.icons", home())))
        .collect();
    themes.insert(ThemeKind::Gtk as u8, dirs(&theme_dirs, "gtk-3.0"));
    themes.insert(ThemeKind::Icons as u8, dirs(&icon_dirs, "index.theme"));
    themes.insert(ThemeKind::Cursor as u8, dirs(&icon_dirs, "cursors"));
    Loaded {
        values,
        fonts,
        themes,
        autostart: read_autostart(),
        launchers: read_launchers(),
    }
}

/// Write a value the way gsettings expects for this kind.
pub(crate) fn write(item: &Item, value: &str) -> bool {
    let arg = match item.kind {
        Kind::Bool | Kind::BoolInverted | Kind::Int(..) => value.to_owned(),
        _ => format!("'{}'", value.replace('\'', "\\'")),
    };
    Command::new("gsettings")
        .args(schema_args(item.schema))
        .args(["set", item.schema, item.key, &arg])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
