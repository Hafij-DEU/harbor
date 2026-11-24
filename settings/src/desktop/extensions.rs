//! Extensions page: list, enable, configure, install from extensions.gnome.org, remove.

use std::process::Command;
use std::sync::mpsc::{self};

use eframe::egui::{self, Context, RichText, Ui};

use super::*;
use crate::app::App;
use crate::icons::Icon;
use crate::theme::regular;
use crate::widgets::{self, Right, ROW_PAD};

#[derive(Clone)]
pub struct Extension {
    pub uuid: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub state: i64,
    pub user: bool,
    pub has_prefs: bool,
    pub version: String,
    pub path: String,
    pub schema: String,
    /// Version on disk when it differs from the running one (update waits for a new login).
    pub pending: String,
}

#[derive(Clone)]
pub struct Opt {
    pub key: String,
    pub label: String,
    pub on: bool,
}

/// Run `gnome-extensions <verb> <uuid>` off the UI thread.
pub(crate) fn ext_action(app: &mut App, ctx: &Context, verb: &'static str, uuid: String) {
    spawn(app, &ctx, move || {
        let mut res = run("gnome-extensions", &[verb, &uuid]);
        let mut note = String::new();
        // GNOME shows one extension settings window at a time; a hidden
        // or stuck one blocks every later open until its service restarts.
        if verb == "prefs"
            && res.as_ref().is_err_and(|e| e.contains("Already showing"))
            && restart_prefs_service()
        {
            res = run("gnome-extensions", &[verb, &uuid]);
            note = "Closed the other extension settings window".to_owned();
        }
        Done::Message(match verb {
            "prefs" => res
                .map(|_| note)
                .map_err(|e| format!("Could not open settings: {e}")),
            _ => res.map(|_| {
                format!(
                    "Extension turned {}",
                    if verb == "enable" { "on" } else { "off" }
                )
            }),
        })
    });
}

pub(crate) const LIST_PY: &str = r#"
import json
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
r = bus.call_sync('org.gnome.Shell.Extensions', '/org/gnome/Shell/Extensions',
    'org.gnome.Shell.Extensions', 'ListExtensions', None, None, 0, 5000, None).unpack()[0]
def ver(d):
    v = d.get('version-name', d.get('version', ''))
    if isinstance(v, float) and v.is_integer():
        v = int(v)
    return str(v)
def disk(path):
    try:
        return ver(json.load(open(path + '/metadata.json')))
    except Exception:
        return ''
s = Gio.Settings.new('org.gnome.shell')
on = set(s.get_strv('enabled-extensions'))
out = [dict(uuid=u, name=d.get('name', u), description=d.get('description', ''),
    enabled=u in on, state=int(d.get('state', 0)), user=int(d.get('type', 2)) == 2,
    has_prefs=bool(d.get('hasPrefs', False)), version=ver(d),
    path=d.get('path', ''), schema=d.get('settings-schema', ''), disk=disk(d.get('path', '')))
    for u, d in r.items()]
print(json.dumps(dict(user_on=not s.get_boolean('disable-user-extensions'), list=out)))
"#;

/// argv: schema dir ('' = installed system-wide), schema id [, key, true|false]
pub(crate) const OPTS_PY: &str = r#"
import json, os, sys
from gi.repository import Gio
d, sid = sys.argv[1], sys.argv[2]
default = Gio.SettingsSchemaSource.get_default()
src = Gio.SettingsSchemaSource.new_from_directory(d, default, False) if d and os.path.isdir(d) else default
sch = src.lookup(sid, True)
s = Gio.Settings.new_full(sch, None, None)
if len(sys.argv) > 4:
    s.set_boolean(sys.argv[3], sys.argv[4] == 'true')
    Gio.Settings.sync()
print(json.dumps([dict(key=k, label=(sch.get_key(k).get_summary() or k), on=s.get_boolean(k))
    for k in sorted(sch.list_keys()) if sch.get_key(k).get_value_type().dup_string() == 'b']))
"#;

/// Fusion Desk features as simple switches (the rest stays in its own settings).
pub(crate) const FUSION_KEYS: &[(&str, &str)] = &[
    ("menu-visible", "Start button"),
    ("activities-visible", "Activities button"),
    ("taskbar-visible", "Taskbar"),
    ("windows-extras-in-fusion", "Search and Task View buttons"),
    ("settings-button", "Settings on the taskbar"),
    ("apps-visible", "Show Applications button"),
    ("title-visible", "Window title"),
    ("workspaces-visible", "Workspace switcher"),
    ("media-visible", "Media controls"),
    ("sysmon-visible", "System monitor"),
    ("clipboard-visible", "Clipboard history icon"),
    ("clipboard-super-v", "Super+V opens the clipboard"),
    ("caffeine-visible", "Caffeine (keep awake)"),
    ("screenshot-visible", "Screenshot button"),
    ("apphub-visible", "App Hub button"),
    ("overflow-visible", "Hidden tray icons arrow"),
    ("desktop-visible", "Show Desktop strip"),
    ("battery-standalone", "Battery next to the system menu"),
    ("icons-follow-accent", "Icons in the accent colour"),
    ("deepin-control-button", "Control centre"),
    ("deepin-organizer", "Desktop organizer"),
    ("deepin-floating-taskbar", "Floating taskbar"),
    ("wallpaper-blur", "Blur the wallpaper behind windows"),
    ("overview-blur", "Blurred overview"),
    ("window-blur", "See-through blurred windows"),
    ("add-to-desktop", "Add to Desktop in app menus"),
    ("charge-limit", "Battery charge limit"),
    (
        "avoid-duplicates",
        "Hide items other extensions already show",
    ),
];

/// A compact on/off line used inside an extension's options list.
pub(crate) fn option_row(
    ui: &mut egui::Ui,
    pal: &crate::theme::Palette,
    label: &str,
    on: &mut bool,
) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(egui::vec2(4.0, 1.0)), pal.corner(4), pal.hover);
    }
    let left = rect.left() + ROW_PAD + 54.0;
    let sw = egui::Rect::from_center_size(
        egui::pos2(rect.right() - ROW_PAD - 20.0, rect.center().y),
        egui::vec2(40.0, 20.0),
    );
    let max_w = sw.left() - 16.0 - left;
    let mut text = label.to_owned();
    let font = regular(13.0);
    while text.chars().count() > 8
        && ui
            .painter()
            .layout_no_wrap(text.clone(), font.clone(), pal.text)
            .size()
            .x
            > max_w
    {
        text = text
            .chars()
            .take(text.chars().count() - 2)
            .collect::<String>()
            .trim_end()
            .to_owned()
            + "…";
        text = text.replace("……", "…");
    }
    ui.painter().text(
        egui::pos2(left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        pal.text,
    );
    let t = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("sw"), *on, 0.11);
    widgets::paint_switch(ui.painter(), sw, t, pal, pal.accent);
    let clicked = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
        .clicked();
    if clicked {
        *on = !*on;
    }
    clicked
}

pub(crate) fn load_options(
    app: &mut App,
    ctx: &Context,
    e: &Extension,
    set: Option<(String, bool)>,
) {
    let dir = if e.path.is_empty() {
        String::new()
    } else {
        format!("{}/schemas", e.path)
    };
    let (uuid, schema) = (e.uuid.clone(), e.schema.clone());
    spawn(app, ctx, move || {
        let mut args = vec!["-c".to_owned(), OPTS_PY.to_owned(), dir, schema];
        if let Some((k, v)) = set {
            args.push(k);
            args.push(v.to_string());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut opts: Vec<Opt> = run("python3", &refs)
            .ok()
            .and_then(|out| serde_json::from_str::<Vec<serde_json::Value>>(&out).ok())
            .unwrap_or_default()
            .iter()
            .map(|v| Opt {
                key: v["key"].as_str().unwrap_or("").to_owned(),
                label: v["label"].as_str().unwrap_or("").to_owned(),
                on: v["on"].as_bool().unwrap_or(false),
            })
            .collect();
        if uuid.starts_with("fusion-desk@") {
            opts = FUSION_KEYS
                .iter()
                .filter_map(|(k, label)| {
                    opts.iter().find(|o| o.key == *k).map(|o| Opt {
                        label: (*label).to_owned(),
                        ..o.clone()
                    })
                })
                .collect();
        }
        Done::Options(uuid, opts)
    });
}

pub(crate) const REMOTE_PY: &str = r#"
import sys
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
r = bus.call_sync('org.gnome.Shell.Extensions', '/org/gnome/Shell/Extensions',
    'org.gnome.Shell.Extensions', 'InstallRemoteExtension', GLib.Variant('(s)', (sys.argv[1],)),
    None, 0, 600000, None).unpack()[0]
print(r)
"#;

pub(crate) const SEARCH_PY: &str = r#"
import json, sys, urllib.parse, urllib.request
q = urllib.parse.urlencode({'search': sys.argv[1], 'shell_version': sys.argv[2], 'n_per_page': 20})
req = urllib.request.Request('https://extensions.gnome.org/extension-query/?' + q, headers={'User-Agent': 'Settings'})
d = json.load(urllib.request.urlopen(req, timeout=15))
print(json.dumps([dict(uuid=e['uuid'], name=e['name'], description=e['description'].split('\n')[0][:160])
    for e in d.get('extensions', [])]))
"#;

pub(crate) fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    match Command::new(cmd).args(args).output() {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_owned()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr)
            .lines()
            .last()
            .unwrap_or("failed")
            .to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

/// Stop the org.gnome.Shell.Extensions service (D-Bus starts it again on the next call).
pub(crate) fn restart_prefs_service() -> bool {
    pub(crate) const NAME: &str = "org.gnome.Shell.Extensions";
    let dbus = |method: &str| {
        run(
            "gdbus",
            &[
                "call",
                "--session",
                "-d",
                "org.freedesktop.DBus",
                "-o",
                "/org/freedesktop/DBus",
                "-m",
                &format!("org.freedesktop.DBus.{method}"),
                NAME,
            ],
        )
    };
    let Ok(out) = dbus("GetConnectionUnixProcessID") else {
        return false;
    };
    // gdbus prints "(uint32 1234,)".
    let pid: String = out
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    if pid.is_empty() || !String::from_utf8_lossy(&cmdline).contains(NAME) {
        return false;
    }
    if run("kill", &[&pid]).is_err() {
        return false;
    }
    for _ in 0..30 {
        if dbus("NameHasOwner").is_ok_and(|o| o.contains("false")) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

pub(crate) fn spawn(app: &mut App, ctx: &Context, job: impl FnOnce() -> Done + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(job());
        ctx.request_repaint();
    });
    app.desk.jobs.push(rx);
}

pub(crate) fn refresh_extensions(app: &mut App, ctx: &Context) {
    app.desk.ext_loading = true;
    spawn(app, ctx, || {
        let list = run("python3", &["-c", LIST_PY])
            .ok()
            .and_then(|out| serde_json::from_str::<serde_json::Value>(&out).ok());
        let Some(v) = list else {
            return Done::Extensions(Vec::new());
        };
        let mut exts: Vec<Extension> = v["list"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| Extension {
                        uuid: e["uuid"].as_str().unwrap_or("").to_owned(),
                        name: e["name"].as_str().unwrap_or("").to_owned(),
                        description: e["description"]
                            .as_str()
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("")
                            .to_owned(),
                        enabled: e["enabled"].as_bool().unwrap_or(false),
                        state: e["state"].as_i64().unwrap_or(0),
                        user: e["user"].as_bool().unwrap_or(true),
                        has_prefs: e["has_prefs"].as_bool().unwrap_or(false),
                        version: e["version"].as_str().unwrap_or("").to_owned(),
                        path: e["path"].as_str().unwrap_or("").to_owned(),
                        schema: e["schema"].as_str().unwrap_or("").to_owned(),
                        pending: {
                            let (run, disk) = (
                                e["version"].as_str().unwrap_or(""),
                                e["disk"].as_str().unwrap_or(""),
                            );
                            if disk.is_empty() || disk == run {
                                String::new()
                            } else {
                                disk.to_owned()
                            }
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        exts.sort_by_key(|e| (!e.user, e.name.to_lowercase()));
        // The master switch rides along as a fake first entry.
        let user_on = v["user_on"].as_bool().unwrap_or(true);
        exts.insert(
            0,
            Extension {
                uuid: String::new(),
                name: String::new(),
                description: String::new(),
                enabled: user_on,
                state: 0,
                user: true,
                has_prefs: false,
                version: String::new(),
                path: String::new(),
                schema: String::new(),
                pending: String::new(),
            },
        );
        Done::Extensions(exts)
    });
}

pub(crate) fn poll(app: &mut App, ctx: &Context) {
    let mut done = Vec::new();
    app.desk.jobs.retain(|rx| match rx.try_recv() {
        Ok(d) => {
            done.push(d);
            false
        }
        Err(mpsc::TryRecvError::Empty) => true,
        Err(_) => false,
    });
    for d in done {
        match d {
            Done::Extensions(mut list) => {
                app.desk.ext_loading = false;
                if !list.is_empty() {
                    let master = list.remove(0);
                    app.desk.user_extensions_on = Some(master.enabled);
                }
                // Screenshot hook: open one extension's options straight away.
                if let Ok(want) = std::env::var("WIN11_SETTINGS_EXT_OPEN") {
                    if let Some(e) = list
                        .iter()
                        .find(|e| e.uuid == want && !e.schema.is_empty())
                        .cloned()
                    {
                        if app.desk.open.insert(e.uuid.clone()) {
                            load_options(app, ctx, &e, None);
                        }
                    }
                }
                app.desk.extensions = Some(list);
            }
            Done::Message(Ok(msg)) => {
                if msg.is_empty() {
                    continue;
                }
                app.notify(msg);
                refresh_extensions(app, ctx);
            }
            Done::Message(Err(err)) => app.notify_error(err),
            Done::FusionInstalled(res) => {
                app.desk.fusion_installing = false;
                match res {
                    Ok(msg) => app.notify(msg),
                    Err(err) => app.notify_error(err),
                }
                // Re-read: the Fusion rows appear once its schema is on disk.
                app.desk.loaded = None;
                refresh_extensions(app, ctx);
            }
            Done::Options(uuid, opts) => {
                app.desk.options.insert(uuid, opts);
            }
            Done::Found(res) => {
                app.desk.searching = false;
                match res {
                    Ok(list) => {
                        if list.is_empty() {
                            app.notify("No extensions found");
                        }
                        app.desk.found = list;
                    }
                    Err(err) => app.notify_error(format!("Search failed: {err}")),
                }
            }
        }
    }
}

pub(crate) fn state_text(e: &Extension) -> &'static str {
    match e.state {
        1 => "Running",
        3 => "Error",
        4 => "Not made for this GNOME version",
        5 => "Downloading",
        _ if e.enabled => "On",
        _ => "Off",
    }
}

pub fn extensions_page(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let ctx = ui.ctx().clone();
    poll(app, &ctx);
    widgets::h1(ui, &pal, "Extensions");
    ui.label(
        RichText::new("Turn GNOME Shell extensions on or off, open their settings, add new ones or remove them.")
            .font(regular(12.5))
            .color(pal.text_faint),
    );
    if app.desk.extensions.is_none() && !app.desk.ext_loading {
        refresh_extensions(app, &ctx);
    }

    ui.add_space(14.0);
    let mut master = app.desk.user_extensions_on.unwrap_or(true);
    let mut install_zip = false;
    let mut refresh = false;
    widgets::card(ui, &pal, |ui| {
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Hub,
            "Use extensions",
            "Off turns every extension you installed off at once",
            Right::Switch(&mut master),
        )
        .toggled()
        {
            let ok = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.shell",
                    "disable-user-extensions",
                    if master { "false" } else { "true" },
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                app.desk.user_extensions_on = Some(master);
            }
        }
        widgets::divider(ui, &pal);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            install_zip = widgets::button(
                ui,
                &pal,
                pal.accent,
                "Install from a .zip file",
                Some(Icon::Apps),
            )
            .clicked();
            refresh = widgets::ghost_button(ui, &pal, "Refresh", None).clicked();
        });
        ui.add_space(10.0);
    });
    if refresh {
        refresh_extensions(app, &ctx);
    }
    if install_zip {
        spawn(app, &ctx, || {
            let picked = run(
                "zenity",
                &[
                    "--file-selection",
                    "--title=Choose an extension .zip",
                    "--file-filter=Extensions | *.zip",
                ],
            );
            Done::Message(match picked {
                Ok(path) if !path.is_empty() => {
                    run("gnome-extensions", &["install", "--force", &path])
                        .map(|_| "Extension installed — log out and back in to load it".to_owned())
                }
                Ok(_) => Ok(String::new()),
                // zenity exits 1 when the picker is closed: nothing to report.
                Err(e) if e == "failed" || e.is_empty() => Ok(String::new()),
                Err(e) => Err(format!("Could not open the file picker: {e}")),
            })
        });
    }

    // Get more from extensions.gnome.org (the shell asks before installing).
    section_title(ui, app, "Get extensions");
    let mut search = false;
    let mut install: Option<String> = None;
    widgets::card(ui, &pal, |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.desk.query)
                    .hint_text("Search extensions.gnome.org")
                    .desired_width(ui.available_width() - 130.0),
            );
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if app.desk.searching {
                ui.spinner();
            } else if widgets::button(ui, &pal, pal.accent, "Search", Some(Icon::Search)).clicked()
                || enter
            {
                search = !app.desk.query.trim().is_empty();
            }
        });
        ui.add_space(12.0);
        for found in &app.desk.found {
            widgets::divider(ui, &pal);
            let have = app
                .desk
                .extensions
                .as_ref()
                .is_some_and(|l| l.iter().any(|e| e.uuid == found.uuid));
            let right = if have {
                Right::AccentText("Installed")
            } else {
                Right::AccentText("Install")
            };
            let mut desc = found.description.clone();
            if desc.chars().count() > 90 {
                desc = desc
                    .chars()
                    .take(88)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
                    + "…";
            }
            if widgets::row(ui, &pal, pal.accent, Icon::Hub, &found.name, &desc, right).clicked()
                && !have
            {
                install = Some(found.uuid.clone());
            }
        }
    });
    if search {
        app.desk.searching = true;
        let q = app.desk.query.trim().to_owned();
        let shell = run("gnome-shell", &["--version"])
            .ok()
            .and_then(|v| {
                v.split_whitespace()
                    .last()
                    .map(|s| s.split('.').next().unwrap_or("").to_owned())
            })
            .unwrap_or_default();
        spawn(app, &ctx, move || {
            Done::Found(
                run("python3", &["-c", SEARCH_PY, &q, &shell]).and_then(|out| {
                    let v: Vec<serde_json::Value> =
                        serde_json::from_str(&out).map_err(|e| e.to_string())?;
                    Ok(v.iter()
                        .map(|e| Found {
                            uuid: e["uuid"].as_str().unwrap_or("").to_owned(),
                            name: e["name"].as_str().unwrap_or("").to_owned(),
                            description: e["description"].as_str().unwrap_or("").to_owned(),
                        })
                        .collect())
                }),
            )
        });
    }
    if let Some(uuid) = install {
        spawn(app, &ctx, move || {
            Done::Message(match run("python3", &["-c", REMOTE_PY, &uuid]).as_deref() {
                Ok("successful") => Ok("Extension installed".to_owned()),
                Ok("cancelled") => Ok("Install cancelled".to_owned()),
                Ok(other) => Err(format!("Not installed ({other})")),
                Err(e) => Err(e.to_string()),
            })
        });
    }

    let Some(list) = app.desk.extensions.clone() else {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Reading extensions…").color(pal.text_muted));
        });
        return;
    };
    let mut action: Option<(&'static str, String)> = None;
    let mut toggle_open: Option<Extension> = None;
    let mut set_opt: Option<(Extension, String, bool)> = None;
    for (title, user) in [("Installed by you", true), ("Built in", false)] {
        let rows: Vec<&Extension> = list.iter().filter(|e| e.user == user).collect();
        if rows.is_empty() {
            continue;
        }
        section_title(ui, app, title);
        widgets::card(ui, &pal, |ui| {
            for (n, e) in rows.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let mut on = e.enabled;
                let mut sub = format!(
                    "{}{}{}",
                    state_text(e),
                    match (e.version.is_empty(), e.pending.is_empty()) {
                        (true, _) => String::new(),
                        (false, true) => format!(" · version {}", e.version),
                        (false, false) => format!(
                            " · version {} ({} after you sign out)",
                            e.version, e.pending
                        ),
                    },
                    if e.description.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", e.description)
                    }
                );
                if sub.chars().count() > 70 {
                    sub = sub
                        .chars()
                        .take(68)
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                        + "…";
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let extra = if e.has_prefs { 92.0 } else { 0.0 }
                        + if e.user { 92.0 } else { 0.0 }
                        + if e.has_prefs || e.user { ROW_PAD } else { 0.0 };
                    let row = ui.scope(|ui| {
                        ui.set_width(ui.available_width() - extra);
                        widgets::row(
                            ui,
                            &pal,
                            pal.accent,
                            Icon::Hub,
                            &e.name,
                            &sub,
                            Right::Switch(&mut on),
                        )
                    });
                    if row.inner.toggled() {
                        action = Some((if on { "enable" } else { "disable" }, e.uuid.clone()));
                    }
                    if e.has_prefs && widgets::ghost_button(ui, &pal, "Settings", None).clicked() {
                        action = Some(("prefs", e.uuid.clone()));
                    }
                    if e.user && widgets::ghost_button(ui, &pal, "Remove", None).clicked() {
                        action = Some(("uninstall", e.uuid.clone()));
                    }
                });
                if !e.schema.is_empty() && e.uuid.starts_with("fusion-desk@") {
                    let open = app.desk.open.contains(&e.uuid);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PAD + 42.0);
                        let label = ui.add(
                            egui::Label::new(
                                RichText::new(if open {
                                    "▾ Hide options"
                                } else {
                                    "▸ Options"
                                })
                                .font(regular(12.5))
                                .color(pal.accent),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if label
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            toggle_open = Some((*e).clone());
                        }
                    });
                    ui.add_space(6.0);
                    if open {
                        match app.desk.options.get(&e.uuid) {
                            None => {
                                ui.horizontal(|ui| {
                                    ui.add_space(ROW_PAD + 42.0);
                                    ui.spinner();
                                });
                            }
                            Some(opts) if opts.is_empty() => {
                                ui.horizontal(|ui| {
                                    ui.add_space(ROW_PAD + 42.0);
                                    ui.label(
                                        RichText::new("No on/off options")
                                            .font(regular(12.5))
                                            .color(pal.text_faint),
                                    );
                                });
                            }
                            Some(opts) => {
                                for o in opts {
                                    let mut on = o.on;
                                    if option_row(ui, &pal, &o.label, &mut on) {
                                        set_opt = Some(((*e).clone(), o.key.clone(), on));
                                    }
                                }
                                ui.add_space(6.0);
                            }
                        }
                    }
                }
            }
        });
    }
    if let Some(e) = toggle_open {
        if !app.desk.open.remove(&e.uuid) {
            app.desk.open.insert(e.uuid.clone());
            load_options(app, &ctx, &e, None);
        }
    }
    if let Some((e, key, on)) = set_opt {
        if let Some(list) = app.desk.options.get_mut(&e.uuid) {
            if let Some(o) = list.iter_mut().find(|o| o.key == key) {
                o.on = on;
            }
        }
        load_options(app, &ctx, &e, Some((key, on)));
    }
    if let Some((verb, uuid)) = action {
        if verb == "uninstall" {
            let name = list
                .iter()
                .find(|e| e.uuid == uuid)
                .map(|e| e.name.clone())
                .unwrap_or_default();
            app.confirm_extension_removal(uuid, name);
        } else {
            ext_action(app, &ctx, verb, uuid);
        }
    }
}

/// Called from the confirm dialog: uninstall a user extension.
pub fn uninstall(app: &mut App, ctx: &Context, uuid: String) {
    spawn(app, ctx, move || {
        Done::Message(
            run("gnome-extensions", &["uninstall", &uuid]).map(|_| "Extension removed".to_owned()),
        )
    });
}
