//! Privacy & security > Security pages: overview, firmware updates, camera
//! access, network access (with OpenSnitch for deb apps) and a local ClamAV
//! virus scan. All system work runs through `bridge.py`
//! (`python3 -I -c BRIDGE MODE ...`); root work goes through pkexec and the
//! bridge accepts only its fixed list of operations. Nothing leaves the PC.
//!
//! Every action carries a risk colour (green safe, orange warning, red
//! critical) with the reason, like the Advanced settings tweaks.

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Ui};
use serde_json::Value;

use crate::advanced::{faint, log_card, shared};
use crate::app::{App, Detail, Page, View, SECURITY_PAGES, TWEAK_GROUPS};
use crate::icons::Icon;
use crate::theme::regular;
use crate::widgets::{self, Right};

const BRIDGE: &str = include_str!("bridge.py");

#[derive(Default)]
struct Sec {
    /// Loaded JSON per mode ("overview", "firmware", "apps-camera", ...).
    data: std::collections::HashMap<String, Value>,
    started: std::collections::HashSet<String>,
    loading: usize,
    busy: bool,
    log: Vec<String>,
    folder: String,
    /// An action waiting for a second click (critical / warning confirm).
    confirm: Option<(String, Vec<String>, bool, String)>,
}

type State = Arc<Mutex<Sec>>;

fn cmd(root: bool, args: &[String]) -> Command {
    let mut c = if root {
        let mut c = Command::new("pkexec");
        c.arg("python3");
        c
    } else {
        Command::new("python3")
    };
    c.args(["-I", "-c", BRIDGE]);
    if root {
        c.arg("root");
    }
    c.args(args).stdin(Stdio::null());
    c
}

fn load(state: &State, ctx: &egui::Context, mode: &str, args: &[&str]) {
    let key = std::iter::once(mode).chain(args.iter().copied()).collect::<Vec<_>>().join("-");
    {
        let mut st = state.lock().unwrap();
        st.loading += 1;
    }
    crate::advanced::LOADING.store(true, std::sync::atomic::Ordering::Relaxed);
    let (state, ctx) = (state.clone(), ctx.clone());
    let args: Vec<String> = std::iter::once(mode.to_owned())
        .chain(args.iter().map(|s| s.to_string()))
        .collect();
    std::thread::spawn(move || {
        let out = cmd(false, &args).output();
        let v = out
            .ok()
            .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
            .unwrap_or(Value::Null);
        let mut st = state.lock().unwrap();
        st.data.insert(key, v);
        st.loading -= 1;
        if st.loading == 0 {
            crate::advanced::LOADING.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        ctx.request_repaint();
    });
}

/// Run an action, streaming its output into the log, then reload `reload`.
fn act(state: &State, ctx: &egui::Context, root: bool, args: Vec<String>, reload: Vec<String>) {
    {
        let mut st = state.lock().unwrap();
        if st.busy {
            return;
        }
        st.busy = true;
        st.log.clear();
    }
    let (state, ctx) = (state.clone(), ctx.clone());
    std::thread::spawn(move || {
        let child = cmd(root, &args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let ok = match child {
            Ok(mut c) => {
                if let Some(out) = c.stdout.take() {
                    for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                        state.lock().unwrap().log.push(line);
                        ctx.request_repaint();
                    }
                }
                let status = c.wait().map(|s| s.code()).ok().flatten();
                if let Some(mut e) = c.stderr.take() {
                    let mut s = String::new();
                    let _ = std::io::Read::read_to_string(&mut e, &mut s);
                    if !s.trim().is_empty() {
                        state.lock().unwrap().log.push(s.trim().to_owned());
                    }
                }
                match status {
                    Some(0) => true,
                    Some(126) | Some(127) if root => {
                        state.lock().unwrap().log.push("Cancelled: no administrator rights given.".into());
                        false
                    }
                    _ => false,
                }
            }
            Err(e) => {
                state.lock().unwrap().log.push(format!("could not start: {e}"));
                false
            }
        };
        let mut st = state.lock().unwrap();
        st.busy = false;
        if !ok && !st.log.iter().any(|l| l.starts_with("Cancelled")) {
            st.log.push("Finished with an error.".into());
        }
        for k in reload {
            st.started.remove(&k);
        }
        ctx.request_repaint();
    });
}

fn get<'a>(st: &'a Sec, key: &str) -> Option<&'a Value> {
    st.data.get(key).filter(|v| !v.is_null())
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// Load a page's data once (again after an action asked for a reload).
fn ensure(state: &State, ui: &Ui, mode: &str, args: &[&str]) {
    let key = std::iter::once(mode).chain(args.iter().copied()).collect::<Vec<_>>().join("-");
    let fresh = state.lock().unwrap().started.insert(key);
    if fresh {
        load(state, ui.ctx(), mode, args);
    }
}

fn header(app: &mut App, ui: &mut Ui, i: u8) -> bool {
    let pal = app.pal();
    let title = SECURITY_PAGES[i as usize].0;
    if widgets::h1_back(ui, &pal, title) {
        app.go(View::Page(Page::Privacy));
        return true;
    }
    false
}

fn legend(ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        widgets::risk_chip(ui, "safe", "Safe");
        ui.add_space(10.0);
        widgets::risk_chip(ui, "warning", "Warning: something stops working");
        ui.add_space(10.0);
        widgets::risk_chip(ui, "critical", "Critical: weakens security or can lose data");
    });
    ui.add_space(8.0);
}

pub fn page(app: &mut App, ui: &mut Ui, i: u8) {
    if header(app, ui, i) {
        return;
    }
    let state = shared::<Sec>(ui, "harbor-security");
    match i {
        0 => overview(app, ui, &state),
        1 => firmware(app, ui, &state),
        2 => camera(app, ui, &state),
        3 => network(app, ui, &state),
        _ => virus(app, ui, &state),
    }
    let pal = app.pal();
    let (log, busy) = {
        let st = state.lock().unwrap();
        (st.log.clone(), st.busy)
    };
    if busy {
        ui.add_space(8.0);
        ui.spinner();
    }
    log_card(ui, &pal, &log);
}

/// A pending confirm card; returns true when the user confirmed it.
fn confirm_card(app: &App, ui: &mut Ui, state: &State) {
    let pal = app.pal();
    let pending = state.lock().unwrap().confirm.clone();
    let Some((text, args, root, level)) = pending else {
        return;
    };
    let color = widgets::risk_color(&level);
    let mut go = false;
    let mut cancel = false;
    widgets::card(ui, &pal, |ui| {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "{} {}",
                if level == "critical" { "✖" } else { "⚠" },
                widgets::risk_label(&level)
            ))
            .font(regular(14.0))
            .color(color),
        );
        ui.label(egui::RichText::new(&text).font(regular(12.5)).color(pal.text));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            go = widgets::button(ui, &pal, color, "Continue", None).clicked();
            cancel = widgets::ghost_button(ui, &pal, "Cancel", None).clicked();
        });
        ui.add_space(8.0);
    });
    ui.add_space(10.0);
    if go || cancel {
        state.lock().unwrap().confirm = None;
    }
    if go {
        let reload = state.lock().unwrap().started.iter().cloned().collect();
        act(state, ui.ctx(), root, args, reload);
    }
}

fn ask(state: &State, level: &str, text: &str, root: bool, args: &[&str]) {
    state.lock().unwrap().confirm = Some((
        text.to_owned(),
        args.iter().map(|s| s.to_string()).collect(),
        root,
        level.to_owned(),
    ));
}

fn run_now(state: &State, ui: &Ui, root: bool, args: &[&str]) {
    let reload = state.lock().unwrap().started.iter().cloned().collect();
    act(
        state,
        ui.ctx(),
        root,
        args.iter().map(|s| s.to_string()).collect(),
        reload,
    );
}

fn view_for(key: &str) -> Option<View> {
    if key == "updates" {
        return Some(View::Page(Page::Updates));
    }
    if let Some(i) = TWEAK_GROUPS.iter().position(|g| g.3 == key) {
        return Some(View::Detail(Page::Advanced, Detail::TweakGroup(i as u8)));
    }
    SECURITY_PAGES
        .iter()
        .position(|g| g.2 == key)
        .map(|i| View::Detail(Page::Privacy, Detail::Security(i as u8)))
}

// ------------------------------------------------------------ overview

fn overview(app: &mut App, ui: &mut Ui, state: &State) {
    let pal = app.pal();
    ensure(state, ui, "overview", &[]);
    faint(
        ui,
        &pal,
        "How protected this PC is. Green is fine, orange is worth a look, red needs action. \
         Click a row to fix it.",
    );
    ui.add_space(4.0);
    legend(ui);
    let tiles: Vec<Value> = {
        let st = state.lock().unwrap();
        get(&st, "overview").map_or(Vec::new(), |v| arr(v, "tiles").to_vec())
    };
    if tiles.is_empty() {
        ui.spinner();
        return;
    }
    let worst = ["critical", "warning"]
        .into_iter()
        .find(|l| tiles.iter().any(|t| s(t, "level") == *l))
        .unwrap_or("safe");
    let summary = match worst {
        "critical" => "Action needed",
        "warning" => "Mostly protected, some things to look at",
        _ => "This PC is protected",
    };
    widgets::card(ui, &pal, |ui| {
        widgets::risk_row(ui, &pal, Some(Icon::Shield), summary, "", worst, "", Right::None);
    });
    ui.add_space(12.0);
    widgets::section_label(ui, &pal, "Protection areas");
    let mut go: Option<View> = None;
    widgets::card(ui, &pal, |ui| {
        for (n, t) in tiles.iter().enumerate() {
            if n > 0 {
                widgets::divider(ui, &pal);
            }
            let action = s(t, "action");
            let right = if action.is_empty() { Right::None } else { Right::Chevron };
            let icon = match s(t, "id").as_str() {
                "firewall" | "opensnitch" => Icon::Globe,
                "updates" | "firmware" => Icon::Update,
                "camera" => Icon::Image,
                "virus" => Icon::Search,
                "tpm" | "encryption" => Icon::Storage,
                _ => Icon::Shield,
            };
            if widgets::risk_row(
                ui,
                &pal,
                Some(icon),
                &s(t, "title"),
                &s(t, "state"),
                &s(t, "level"),
                &s(t, "why"),
                right,
            )
            .clicked()
            {
                go = view_for(&action);
            }
        }
    });
    ui.add_space(12.0);
    widgets::section_label(ui, &pal, "What is AppArmor?");
    widgets::card(ui, &pal, |ui| {
        ui.add_space(8.0);
        faint(
            ui,
            &pal,
            "AppArmor is the Linux kernel's app jail. Each confined program (snaps, CUPS, \
             browsers, many services) gets a profile listing the files, devices and network \
             rights it may use; anything else is blocked, so a hacked app cannot read the rest \
             of your files. Ubuntu turns it on by default. Keep it on.",
        );
        ui.add_space(8.0);
    });
    if let Some(v) = go {
        app.go(v);
    }
}

// ------------------------------------------------------------ firmware

fn firmware(app: &mut App, ui: &mut Ui, state: &State) {
    let pal = app.pal();
    ensure(state, ui, "firmware", &[]);
    faint(
        ui,
        &pal,
        "Updates for the BIOS/UEFI, SSD, camera, dock and other devices from the LVFS \
         (fwupd). They often close security holes below the operating system.",
    );
    ui.add_space(4.0);
    confirm_card(app, ui, state);
    let fw = state.lock().unwrap().data.get("firmware").cloned();
    let Some(fw) = fw.filter(|v| !v.is_null()) else {
        ui.spinner();
        return;
    };
    if !b(&fw, "available") {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, "fwupd is not installed (sudo apt install fwupd).")
        });
        return;
    }
    let updates = arr(&fw, "updates").to_vec();
    let age = fw.get("meta_age_days").and_then(Value::as_i64);
    ui.horizontal(|ui| {
        if widgets::ghost_button(ui, &pal, "Check for updates", Some(Icon::Update)).clicked() {
            run_now(state, ui, false, &["fw-refresh"]);
        }
        if !updates.is_empty()
            && widgets::button(ui, &pal, widgets::risk_color("warning"), "Install all", None)
                .clicked()
        {
            ask(
                state,
                "warning",
                "Keep the charger plugged in and do not turn the PC off. A power cut during a \
                 firmware update can leave a device (even the BIOS) unusable. Some updates \
                 install on the next restart.",
                false,
                &["fw-update"],
            );
        }
    });
    ui.add_space(6.0);
    faint(
        ui,
        &pal,
        &match age {
            Some(d) => format!("Update list downloaded {d} day{} ago.", if d == 1 { "" } else { "s" }),
            None => "Update list never downloaded: press Check for updates.".to_owned(),
        },
    );
    ui.add_space(8.0);
    widgets::section_label(ui, &pal, "Available updates");
    widgets::card(ui, &pal, |ui| {
        if updates.is_empty() {
            widgets::risk_row(ui, &pal, Some(Icon::Check), "All firmware is up to date", "", "safe", "", Right::None);
        }
        for (n, u) in updates.iter().enumerate() {
            if n > 0 {
                widgets::divider(ui, &pal);
            }
            widgets::risk_row(
                ui,
                &pal,
                Some(Icon::Update),
                &s(u, "name"),
                &format!("{} → {}  {}", s(u, "from"), s(u, "to"), s(u, "summary")),
                "warning",
                "Do not power off while it installs.",
                Right::None,
            );
        }
    });
    ui.add_space(12.0);
    widgets::section_label(ui, &pal, "Devices");
    widgets::card(ui, &pal, |ui| {
        for (n, d) in arr(&fw, "devices").iter().enumerate() {
            if n > 0 {
                widgets::divider(ui, &pal);
            }
            let sub = format!(
                "{} · version {}{}",
                s(d, "vendor"),
                if s(d, "version").is_empty() { "?".into() } else { s(d, "version") },
                if b(d, "updatable") { " · can be updated" } else { "" }
            );
            widgets::info_row(ui, &pal, &s(d, "name"), &sub, Right::None);
        }
    });
}

// ------------------------------------------------------------ per-app rows

/// Flatpak and snap apps with a switch for `iface` ("camera"/"network").
fn app_rows(app: &App, ui: &mut Ui, state: &State, data: &Value, iface: &str) {
    let pal = app.pal();
    let apps = arr(data, "apps").to_vec();
    widgets::section_label(ui, &pal, "Per app (Flatpak and Snap)");
    widgets::card(ui, &pal, |ui| {
        if apps.is_empty() {
            widgets::empty_note(ui, &pal, "No Flatpak or Snap apps ask for this.");
        }
        for (n, a) in apps.iter().enumerate() {
            if n > 0 {
                widgets::divider(ui, &pal);
            }
            let kind = s(a, "kind");
            let id = s(a, "id");
            let was = b(a, iface);
            let mut on = was;
            let why = match (iface, was) {
                ("camera", true) => "",
                ("camera", false) => "Video calls and photos in this app stop working.",
                (_, true) => "",
                _ => "This app cannot go online: no sync, updates or sign-in.",
            };
            // Apps built without internet (e.g. Fonts) are not a problem.
            let by_design = iface == "network" && !was && !b(a, "network_default");
            let why = if by_design { "Has no internet by design." } else { why };
            let level = if was || by_design { "safe" } else { "warning" };
            let sub = format!(
                "{} · {id}{}",
                if kind == "snap" { "Snap" } else { "Flatpak" },
                if kind == "snap" { " · admin" } else { "" }
            );
            let out = widgets::risk_row(
                ui,
                &pal,
                Some(if iface == "camera" { Icon::Image } else { Icon::Globe }),
                &s(a, "name"),
                &sub,
                level,
                why,
                Right::Switch(&mut on),
            );
            if out.toggled() {
                if kind == "snap" {
                    let op = if on { "snap-connect" } else { "snap-disconnect" };
                    run_now(state, ui, true, &[op, &id, iface]);
                } else {
                    run_now(state, ui, false, &["flatpak-set", &id, iface, if on { "allow" } else { "deny" }]);
                }
            }
        }
    });
}

// ------------------------------------------------------------ camera

fn camera(app: &mut App, ui: &mut Ui, state: &State) {
    let pal = app.pal();
    ensure(state, ui, "apps", &["camera"]);
    faint(
        ui,
        &pal,
        "Turn the camera off for every app, or allow it per app. Deb apps (installed with \
         apt) cannot be limited one by one; use the whole-PC switches for them.",
    );
    ui.add_space(4.0);
    legend(ui);
    confirm_card(app, ui, state);
    let data = state.lock().unwrap().data.get("apps-camera").cloned();
    let Some(data) = data.filter(|v| !v.is_null()) else {
        ui.spinner();
        return;
    };
    let cam = data.get("camera").cloned().unwrap_or(Value::Null);
    widgets::section_label(ui, &pal, "Whole PC");
    widgets::card(ui, &pal, |ui| {
        let soft_was = !b(&cam, "soft_off");
        let mut soft = soft_was;
        let out = widgets::risk_row(
            ui,
            &pal,
            Some(Icon::Image),
            "Let apps use the camera",
            "GNOME privacy switch; respected by Flatpak/portal apps and the GNOME apps",
            "safe",
            if soft_was { "" } else { "Apps that ask through GNOME get no camera." },
            Right::Switch(&mut soft),
        );
        if out.toggled() {
            run_now(state, ui, false, &["camera-soft", if soft { "on" } else { "off" }]);
        }
        widgets::divider(ui, &pal);
        let drv_was = b(&cam, "driver_off");
        let mut drv = drv_was;
        let out = widgets::risk_row(
            ui,
            &pal,
            Some(Icon::Shield),
            "Turn off the camera driver",
            "Blocks the webcam for every app, deb apps included · admin",
            "warning",
            "Every camera app and video call loses the camera until you turn this off again.",
            Right::Switch(&mut drv),
        );
        if out.toggled() {
            run_now(state, ui, true, &[if drv { "camera-driver-off" } else { "camera-driver-on" }]);
        }
    });
    ui.add_space(12.0);
    app_rows(app, ui, state, &data, "camera");
}

// ------------------------------------------------------------ network

fn network(app: &mut App, ui: &mut Ui, state: &State) {
    let pal = app.pal();
    ensure(state, ui, "apps", &["network"]);
    faint(
        ui,
        &pal,
        "Cut the internet for the whole PC or for single apps. Flatpak and Snap apps have a \
         switch here; deb apps are covered by the OpenSnitch app firewall below.",
    );
    ui.add_space(4.0);
    legend(ui);
    confirm_card(app, ui, state);
    let data = state.lock().unwrap().data.get("apps-network").cloned();
    let Some(data) = data.filter(|v| !v.is_null()) else {
        ui.spinner();
        return;
    };
    let net = data.get("network").cloned().unwrap_or(Value::Null);
    widgets::section_label(ui, &pal, "Whole PC");
    widgets::card(ui, &pal, |ui| {
        let mut all = b(&net, "networking");
        let out = widgets::risk_row(
            ui,
            &pal,
            Some(Icon::Globe),
            "Networking",
            "Wi-Fi, Ethernet and mobile together",
            "warning",
            "Turning it off takes the whole PC offline, including updates and file sharing.",
            Right::Switch(&mut all),
        );
        if out.toggled() {
            if all {
                run_now(state, ui, false, &["net-whole", "networking", "on"]);
            } else {
                ask(
                    state,
                    "warning",
                    "The whole PC goes offline until you turn Networking back on here.",
                    false,
                    &["net-whole", "networking", "off"],
                );
            }
        }
        widgets::divider(ui, &pal);
        let mut wifi = b(&net, "wifi");
        let out = widgets::risk_row(
            ui,
            &pal,
            Some(Icon::Wifi),
            "Wi-Fi",
            "Wireless radio",
            "safe",
            "",
            Right::Switch(&mut wifi),
        );
        if out.toggled() {
            run_now(state, ui, false, &["net-whole", "wifi", if wifi { "on" } else { "off" }]);
        }
    });
    ui.add_space(12.0);
    app_rows(app, ui, state, &data, "network");
    ui.add_space(12.0);

    let sn = data.get("opensnitch").cloned().unwrap_or(Value::Null);
    widgets::section_label(ui, &pal, "Deb apps: OpenSnitch app firewall");
    widgets::card(ui, &pal, |ui| {
        if !b(&sn, "installed") {
            let out = widgets::risk_row(
                ui,
                &pal,
                Some(Icon::Shield),
                "Install OpenSnitch",
                "Asks you before any program connects out; allow or deny per app · admin",
                "warning",
                "Until you answer its pop-ups, new connections wait; a wrong Deny can break an app's updates.",
                Right::AccentText("Install"),
            );
            if out.clicked() {
                run_now(state, ui, true, &["install-opensnitch"]);
            }
        } else {
            let running = b(&sn, "running");
            let mut on = running;
            let out = widgets::risk_row(
                ui,
                &pal,
                Some(Icon::Shield),
                "OpenSnitch app firewall",
                if running { "Running: new connections ask first" } else { "Stopped" },
                if running { "safe" } else { "warning" },
                if running { "" } else { "Deb apps can connect anywhere without asking." },
                Right::Switch(&mut on),
            );
            if out.toggled() {
                run_now(state, ui, true, &[if on { "opensnitch-start" } else { "opensnitch-stop" }]);
            }
            widgets::divider(ui, &pal);
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Apps,
                "Open OpenSnitch rules",
                "See and change which apps may connect",
                Right::Chevron,
            )
            .clicked()
            {
                if let Ok(mut c) = cmd(false, &["opensnitch-ui".into()]).spawn() {
                    std::thread::spawn(move || {
                        let _ = c.wait();
                    });
                }
            }
        }
    });
}

// ------------------------------------------------------------ virus scan

fn virus(app: &mut App, ui: &mut Ui, state: &State) {
    let pal = app.pal();
    ensure(state, ui, "clam", &[]);
    faint(
        ui,
        &pal,
        "Scans files with ClamAV on this PC. Nothing is uploaded anywhere. Linux malware is \
         rare; scans mostly catch Windows viruses in downloads and USB sticks.",
    );
    ui.add_space(4.0);
    legend(ui);
    confirm_card(app, ui, state);
    let data = state.lock().unwrap().data.get("clam").cloned();
    let Some(c) = data.filter(|v| !v.is_null()) else {
        ui.spinner();
        return;
    };
    let busy = state.lock().unwrap().busy;
    if !b(&c, "installed") {
        widgets::card(ui, &pal, |ui| {
            let out = widgets::risk_row(
                ui,
                &pal,
                Some(Icon::Search),
                "Install ClamAV",
                "Free open-source scanner plus daily definition updates (about 250 MB) · admin",
                "safe",
                "The update service uses some memory in the background.",
                Right::AccentText("Install"),
            );
            if out.clicked() {
                run_now(state, ui, true, &["install-clamav"]);
            }
        });
        return;
    }
    let age = c.get("db_age_days").and_then(Value::as_i64);
    widgets::section_label(ui, &pal, "Definitions");
    widgets::card(ui, &pal, |ui| {
        let (level, why) = match age {
            None => ("critical", "No virus definitions yet: scans cannot run. Update now."),
            Some(d) if d > 7 => ("warning", "Definitions are over a week old; new threats are missed."),
            _ => ("safe", ""),
        };
        let sub = match age {
            Some(d) => format!(
                "Updated {d} day{} ago · auto-update {}",
                if d == 1 { "" } else { "s" },
                if b(&c, "freshclam") { "on" } else { "off" }
            ),
            None => "Not downloaded".to_owned(),
        };
        if widgets::risk_row(
            ui,
            &pal,
            Some(Icon::Update),
            "Virus definitions",
            &sub,
            level,
            why,
            Right::AccentText("Update now"),
        )
        .clicked()
            && !busy
        {
            run_now(state, ui, true, &["clam-update"]);
        }
    });
    ui.add_space(12.0);
    widgets::section_label(ui, &pal, "Scan");
    let home = std::env::var("HOME").unwrap_or_default();
    widgets::card(ui, &pal, |ui| {
        let quick = [
            format!("{home}/Downloads"),
            format!("{home}/Desktop"),
            format!("{home}/Documents"),
        ];
        let q: Vec<&str> = quick.iter().map(String::as_str).collect();
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Search,
            "Quick scan",
            "Downloads, Desktop and Documents",
            Right::AccentText("Scan"),
        )
        .clicked()
            && !busy
        {
            let mut a = vec!["scan"];
            a.extend(q.iter());
            run_now(state, ui, false, &a);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Accounts,
            "Full home scan",
            "Every file in your home folder (can take a long time)",
            Right::AccentText("Scan"),
        )
        .clicked()
            && !busy
        {
            run_now(state, ui, false, &["scan", &home]);
        }
        widgets::divider(ui, &pal);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let mut st = state.lock().unwrap();
            ui.add(
                egui::TextEdit::singleline(&mut st.folder)
                    .hint_text("Folder or USB stick, e.g. /media/you/USB")
                    .desired_width(ui.available_width() - 120.0),
            );
            let folder = st.folder.trim().to_owned();
            drop(st);
            if widgets::ghost_button(ui, &pal, "Scan folder", None).clicked()
                && !busy
                && !folder.is_empty()
            {
                run_now(state, ui, false, &["scan", &folder]);
            }
        });
        ui.add_space(8.0);
    });

    let last = c.get("last_scan").cloned().unwrap_or(Value::Null);
    let threats = arr(&last, "threats").to_vec();
    if !last.is_null() {
        ui.add_space(12.0);
        widgets::section_label(ui, &pal, &format!("Last scan · {}", s(&last, "when")));
        widgets::card(ui, &pal, |ui| {
            if threats.is_empty() {
                widgets::risk_row(ui, &pal, Some(Icon::Check), "No threats found", "", "safe", "", Right::None);
            }
            for (n, t) in threats.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let path = s(t, "path");
                widgets::risk_row(
                    ui,
                    &pal,
                    Some(Icon::Shield),
                    &s(t, "sig"),
                    &path,
                    "critical",
                    "Do not open this file. Quarantine keeps a locked copy you can restore.",
                    Right::None,
                );
                ui.horizontal(|ui| {
                    ui.add_space(56.0);
                    if widgets::button(ui, &pal, pal.accent, "Quarantine", None).clicked() && !busy {
                        run_now(state, ui, false, &["quarantine", &path]);
                    }
                    if widgets::ghost_button(ui, &pal, "Delete", Some(Icon::Trash)).clicked() && !busy {
                        ask(
                            state,
                            "critical",
                            &format!("{path} is deleted for good. If ClamAV was wrong (a false alarm), the file is lost. Quarantine is the safer choice."),
                            false,
                            &["delete", &path],
                        );
                    }
                });
                ui.add_space(6.0);
            }
        });
    }

    let quar = arr(&c, "quarantine").to_vec();
    if !quar.is_empty() {
        ui.add_space(12.0);
        widgets::section_label(ui, &pal, "Quarantine");
        widgets::card(ui, &pal, |ui| {
            for (n, q) in quar.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let file = s(q, "file");
                widgets::info_row(ui, &pal, &s(q, "from"), &format!("Quarantined {}", s(q, "when")), Right::None);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    if widgets::ghost_button(ui, &pal, "Restore", None).clicked() && !busy {
                        ask(
                            state,
                            "critical",
                            "The file goes back where it was. Only restore it if you are sure it is safe.",
                            false,
                            &["q-restore", &file],
                        );
                    }
                    if widgets::ghost_button(ui, &pal, "Delete for good", Some(Icon::Trash)).clicked() && !busy {
                        run_now(state, ui, false, &["q-delete", &file]);
                    }
                });
                ui.add_space(6.0);
            }
        });
    }
}
