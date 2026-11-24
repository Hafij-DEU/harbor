//! Small wrappers around gsettings/dconf reads and writes.

use std::process::Command;

use super::*;

thread_local! {
    /// Every GSettings value, filled once per load by `dconf_begin`.
    pub(crate) static DCONF: std::cell::RefCell<Option<std::collections::HashMap<String, String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Read every schema default plus every user override in two processes, then
/// answer all later `gsettings_text` calls from memory. `Info::load` used to
/// spawn one `gsettings` process per key (40+), which dominated startup time.
pub(crate) fn dconf_begin() {
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if command_exists("gsettings") {
        for line in sh("gsettings list-recursively 2>/dev/null").lines() {
            let mut it = line.splitn(3, ' ');
            let (Some(schema), Some(key), Some(val)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            map.insert(
                format!("/{}/{}", schema.replace('.', "/"), key),
                val.trim().to_owned(),
            );
        }
    }
    if command_exists("dconf") {
        let dump = sh("dconf dump / 2>/dev/null");
        let mut section = String::new();
        for line in dump.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim_matches('/').to_owned();
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            let path = if section.is_empty() {
                format!("/{key}")
            } else {
                format!("/{section}/{key}")
            };
            map.insert(path, val.trim().to_owned());
        }
    }
    DCONF.with(|c| *c.borrow_mut() = Some(map));
    DESKTOP_ENTRIES.with(|c| *c.borrow_mut() = None);
    XDG_DEFAULTS.with(|c| *c.borrow_mut() = None);
}

pub(crate) fn gsettings_text(schema: &str, key: &str) -> String {
    let path = format!("/{}/{}", schema.replace('.', "/"), key);
    if let Some(v) = DCONF.with(|c| c.borrow().as_ref().and_then(|m| m.get(&path).cloned())) {
        return v;
    }
    sh(&format!("gsettings get {} {} 2>/dev/null", schema, key))
}

pub(crate) fn gs_bool(schema: &str, key: &str) -> Option<bool> {
    let raw = gsettings_text(schema, key);
    if raw.is_empty() {
        None
    } else {
        Some(raw.contains("true"))
    }
}

pub(crate) fn gs_uint(schema: &str, key: &str) -> Option<u32> {
    gsettings_text(schema, key)
        .split_whitespace()
        .last()
        .and_then(|v| v.parse::<u32>().ok())
}

pub(crate) fn gs_set_bool(schema: &str, key: &str, value: bool) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, if value { "true" } else { "false" }])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(crate) fn gsettings_set_uint(schema: &str, key: &str, value: u32) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, &format!("uint32 {value}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(crate) fn gsettings_set_enum(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(crate) fn gsettings_set_string(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, &format!("'{value}'")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Write a GVariant literal (arrays, tuples) without adding quotes.
pub(crate) fn gsettings_set_raw(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
