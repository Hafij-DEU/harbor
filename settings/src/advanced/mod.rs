//! Advanced settings (own left-menu page: Glint's system tweaks) and
//! Network & internet > Advanced network settings (DNS).
//!
//! The catalogue and the root work come from Glint (`glint_core.py`,
//! `tweak_helper.py`, copied here) through `bridge.py`, which runs as
//! `python3 -I -c BRIDGE CORE HELPER MODE ...`; root tweaks go through one
//! pkexec prompt and the bridge accepts only catalogue operations.
//! Page state lives in egui memory, so `App` needs no new fields.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Id, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::theme::{regular, Palette};
use crate::widgets::{self, Right};

/// A page is still reading its data (the screenshot mode waits for it).
pub static LOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn loading(on: bool) {
    LOADING.store(on, std::sync::atomic::Ordering::Relaxed);
}

const BRIDGE: &str = include_str!("bridge.py");
const CORE: &str = include_str!("glint_core.py");
const HELPER: &str = include_str!("tweak_helper.py");

fn bridge(prefix: &[&str], args: &[String]) -> Command {
    let mut cmd = Command::new(prefix[0]);
    cmd.args(&prefix[1..])
        .args(["python3", "-I", "-c", BRIDGE, CORE, HELPER])
        .args(args)
        .stdin(Stdio::null());
    cmd
}

/// Run the bridge, returning (success, stdout+stderr).
fn run(prefix: &[&str], args: &[String]) -> (bool, String) {
    match bridge(prefix, args).output() {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            (o.status.success(), text)
        }
        Err(e) => (false, format!("could not start python3: {e}")),
    }
}

fn shared<T: Default + Send + 'static>(ui: &Ui, key: &str) -> Arc<Mutex<T>> {
    ui.ctx()
        .data_mut(|d| d.get_temp_mut_or_default::<Arc<Mutex<T>>>(Id::new(key)).clone())
}

fn faint(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(regular(12.0))
            .color(pal.text_faint),
    );
}

fn log_card(ui: &mut Ui, pal: &Palette, log: &[String]) {
    if log.is_empty() {
        return;
    }
    ui.add_space(12.0);
    widgets::section_label(ui, pal, "Last run");
    widgets::card(ui, pal, |ui| {
        ui.add_space(6.0);
        let start = log.len().saturating_sub(14);
        for line in &log[start..] {
            ui.label(
                egui::RichText::new(line)
                    .font(egui::FontId::monospace(11.0))
                    .color(pal.text_faint),
            );
        }
        ui.add_space(6.0);
    });
}

// ---------------------------------------------------------------- tweaks

#[derive(Clone, Default)]
struct Tweak {
    id: String,
    cat: String,
    title: String,
    desc: String,
    warn: String,
    rec: bool,
    root: bool,
    applied: bool,
    want: bool,
}

#[derive(Default)]
struct TweakState {
    started: bool,
    busy: bool,
    error: Option<String>,
    cats: Vec<(String, String, String)>,
    tweaks: Vec<Tweak>,
    log: Vec<String>,
}

fn s(v: &serde_json::Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned()
}

fn b(v: &serde_json::Value, k: &str) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
}

fn load_tweaks(state: Arc<Mutex<TweakState>>, ctx: egui::Context) {
    state.lock().unwrap().busy = true;
    loading(true);
    std::thread::spawn(move || {
        let (ok, text) = run(&["env"], &["tweaks".to_owned()]);
        loading(false);
        let parsed = serde_json::from_str::<serde_json::Value>(text.trim()).ok();
        let mut st = state.lock().unwrap();
        st.busy = false;
        match (ok, parsed) {
            (true, Some(v)) => {
                st.error = None;
                st.cats = v["cats"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|c| (s(c, "id"), s(c, "title"), s(c, "desc")))
                    .collect();
                st.tweaks = v["tweaks"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|t| Tweak {
                        id: s(t, "id"),
                        cat: s(t, "cat"),
                        title: s(t, "title"),
                        desc: s(t, "desc"),
                        warn: s(t, "warn"),
                        rec: b(t, "rec"),
                        root: b(t, "root"),
                        applied: b(t, "applied"),
                        want: b(t, "applied"),
                    })
                    .collect();
            }
            _ => st.error = Some(format!("Could not read the tweaks: {}", text.trim())),
        }
        ctx.request_repaint();
    });
}

fn apply_tweaks(state: Arc<Mutex<TweakState>>, ctx: egui::Context) {
    let (user, root): (Vec<String>, Vec<String>) = {
        let mut st = state.lock().unwrap();
        st.busy = true;
        st.log.clear();
        let mut user = vec!["user".to_owned()];
        let mut root = vec!["root".to_owned()];
        for t in st.tweaks.iter().filter(|t| t.want != t.applied) {
            if t.root {
                let verb = if t.want { "tweak-apply" } else { "tweak-revert" };
                root.push(format!("{verb}:{}", t.id));
            } else {
                let verb = if t.want { "apply" } else { "revert" };
                user.push(format!("{verb}:{}", t.id));
            }
        }
        (user, root)
    };
    std::thread::spawn(move || {
        let mut log = Vec::new();
        let mut ok = true;
        if user.len() > 1 {
            let (good, text) = run(&["env"], &user);
            ok &= good;
            log.extend(text.lines().map(str::to_owned));
        }
        if root.len() > 1 {
            log.push("Asking for administrator rights…".to_owned());
            let (good, text) = run(&["pkexec"], &root);
            ok &= good;
            log.extend(text.lines().map(str::to_owned));
        }
        log.push(if ok {
            "Done.".to_owned()
        } else {
            "Some changes failed (see above).".to_owned()
        });
        state.lock().unwrap().log = log;
        load_tweaks(state, ctx);
    });
}

pub fn advanced(app: &mut App, ui: &mut Ui) {
    advanced_group(app, ui, None);
}

/// Advanced settings overview (`None`: one row per group) or one group's
/// tweaks. Pending switches are shared, so Apply works from any of them.
pub fn advanced_group(app: &mut App, ui: &mut Ui, group: Option<u8>) {
    use crate::app::{Detail, Page, View, TWEAK_GROUPS};
    let pal = app.pal();
    let wanted = group.and_then(|i| TWEAK_GROUPS.get(i as usize));
    match wanted {
        Some(g) => {
            if widgets::h1_back(ui, &pal, g.1) {
                app.go(View::Page(Page::Advanced));
                return;
            }
        }
        None => widgets::h1(ui, &pal, "Advanced settings"),
    }
    let mut open_group: Option<u8> = None;
    let state = shared::<TweakState>(ui, "harbor-advanced-tweaks");
    {
        let mut st = state.lock().unwrap();
        if !st.started {
            st.started = true;
            drop(st);
            load_tweaks(state.clone(), ui.ctx().clone());
        }
    }
    let mut st = state.lock().unwrap();
    faint(
        ui,
        &pal,
        "System tweaks for privacy, speed and comfort. Turn switches on or off, then Apply. \
         Rows marked admin ask for your password once. Every tweak can be turned back off.",
    );
    ui.add_space(8.0);

    let pending = st.tweaks.iter().filter(|t| t.want != t.applied).count();
    let mut apply = false;
    let mut reload = false;
    ui.horizontal(|ui| {
        let label = if pending > 0 {
            format!("Apply {pending} change{}", if pending == 1 { "" } else { "s" })
        } else {
            "Apply".to_owned()
        };
        if widgets::button(ui, &pal, pal.accent, &label, Some(Icon::Check)).clicked()
            && pending > 0
            && !st.busy
        {
            apply = true;
        }
        if widgets::ghost_button(ui, &pal, "Select recommended", Some(Icon::Sparkle)).clicked() {
            for t in st.tweaks.iter_mut().filter(|t| t.rec) {
                t.want = true;
            }
        }
        if pending > 0 && widgets::ghost_button(ui, &pal, "Undo changes", None).clicked() {
            for t in st.tweaks.iter_mut() {
                t.want = t.applied;
            }
        }
        if widgets::ghost_button(ui, &pal, "Refresh", Some(Icon::Update)).clicked() && !st.busy {
            reload = true;
        }
        if st.busy {
            ui.spinner();
        }
    });
    ui.add_space(10.0);

    if let Some(err) = st.error.clone() {
        widgets::card(ui, &pal, |ui| widgets::empty_note(ui, &pal, &err));
    }
    let cats = st.cats.clone();
    if wanted.is_none() && st.error.is_none() {
        // Overview: one row per group instead of every switch at once.
        widgets::section_label(ui, &pal, "Groups");
        widgets::card(ui, &pal, |ui| {
            let mut first = true;
            for (i, g) in TWEAK_GROUPS.iter().enumerate() {
                let rows: Vec<&Tweak> = st.tweaks.iter().filter(|t| t.cat == g.0).collect();
                if rows.is_empty() {
                    continue;
                }
                if !first {
                    widgets::divider(ui, &pal);
                }
                first = false;
                let on = rows.iter().filter(|t| t.applied).count();
                let pending = rows.iter().filter(|t| t.want != t.applied).count();
                let desc = cats
                    .iter()
                    .find(|c| c.0 == g.0)
                    .map_or(String::new(), |c| c.2.clone());
                let mut sub = format!("{desc} · {on} of {} on", rows.len());
                if pending > 0 {
                    sub = format!("{sub} · {pending} not applied yet");
                }
                if widgets::row(ui, &pal, pal.accent, g.2, g.1, &sub, Right::Chevron).clicked() {
                    open_group = Some(i as u8);
                }
            }
        });
    }
    for (cid, title, desc) in &cats {
        if wanted.map(|g| g.0) != Some(cid.as_str()) {
            continue;
        }
        let idx: Vec<usize> = (0..st.tweaks.len())
            .filter(|&i| &st.tweaks[i].cat == cid)
            .collect();
        if idx.is_empty() {
            continue;
        }
        let _ = title;
        faint(ui, &pal, desc);
        ui.add_space(4.0);
        widgets::card(ui, &pal, |ui| {
            for (n, &i) in idx.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let t = &mut st.tweaks[i];
                let mut sub = t.desc.clone();
                let mut tags = Vec::new();
                if t.rec {
                    tags.push("recommended");
                }
                if t.root {
                    tags.push("admin");
                }
                if t.want != t.applied {
                    tags.push("not applied yet");
                }
                if !tags.is_empty() {
                    sub = format!("{sub} · {}", tags.join(" · "));
                }
                if !t.warn.is_empty() {
                    sub = format!("{sub}\n⚠ {}", t.warn);
                }
                let title = t.title.clone();
                widgets::info_row(ui, &pal, &title, &sub, Right::Switch(&mut t.want));
            }
        });
        ui.add_space(12.0);
    }
    let log = st.log.clone();
    drop(st);
    log_card(ui, &pal, &log);
    if let Some(i) = open_group {
        app.go(View::Detail(Page::Advanced, Detail::TweakGroup(i)));
    }
    if apply {
        apply_tweaks(state.clone(), ui.ctx().clone());
    } else if reload {
        load_tweaks(state, ui.ctx().clone());
    }
}

// ---------------------------------------------------------------- DNS

#[derive(Clone, Default)]
struct Preset {
    id: String,
    title: String,
    desc: String,
    servers: Vec<String>,
}

#[derive(Default)]
struct DnsState {
    started: bool,
    busy: bool,
    conn: Option<(String, String, String)>,
    current: Vec<String>,
    presets: Vec<Preset>,
    latency: std::collections::HashMap<String, Option<f64>>,
    custom: String,
    log: Vec<String>,
}

fn str_vec(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn load_dns(state: Arc<Mutex<DnsState>>, ctx: egui::Context) {
    state.lock().unwrap().busy = true;
    loading(true);
    std::thread::spawn(move || {
        let (ok, text) = run(&["env"], &["dns-info".to_owned()]);
        loading(false);
        let mut st = state.lock().unwrap();
        st.busy = false;
        match serde_json::from_str::<serde_json::Value>(text.trim()) {
            Ok(v) if ok => {
                let c = str_vec(&v["conn"]);
                st.conn = (c.len() == 3).then(|| (c[0].clone(), c[1].clone(), c[2].clone()));
                st.current = str_vec(&v["current"]);
                st.presets = v["presets"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|p| Preset {
                        id: s(p, "id"),
                        title: s(p, "title"),
                        desc: s(p, "desc"),
                        servers: str_vec(&p["servers"]),
                    })
                    .collect();
            }
            _ => st.log = vec![format!("Could not read DNS settings: {}", text.trim())],
        }
        ctx.request_repaint();
    });
}

fn dns_job(state: Arc<Mutex<DnsState>>, ctx: egui::Context, args: Vec<String>, reload: bool) {
    state.lock().unwrap().busy = true;
    std::thread::spawn(move || {
        let (ok, text) = run(&["env"], &args);
        {
            let mut st = state.lock().unwrap();
            st.busy = false;
            if args[0] == "dns-test" {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                    if let Some(map) = v.as_object() {
                        for (k, ms) in map {
                            st.latency.insert(k.clone(), ms.as_f64());
                        }
                    }
                }
            } else {
                let mut log: Vec<String> = text.lines().map(str::to_owned).collect();
                log.push(if ok { "Done." } else { "Failed." }.to_owned());
                st.log = log;
            }
        }
        if reload {
            load_dns(state, ctx);
        } else {
            ctx.request_repaint();
        }
    });
}

pub fn advanced_network(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Advanced network settings") {
        app.back();
        return;
    }
    let state = shared::<DnsState>(ui, "harbor-advanced-dns");
    {
        let mut st = state.lock().unwrap();
        if !st.started {
            st.started = true;
            drop(st);
            load_dns(state.clone(), ui.ctx().clone());
        }
    }
    let ctx = ui.ctx().clone();
    let mut st = state.lock().unwrap();
    let mut job: Option<(Vec<String>, bool)> = None;

    widgets::section_label(ui, &pal, "DNS servers");
    let conn_text = match &st.conn {
        Some((_, name, dev)) => format!("{name} ({dev})"),
        None => "No active connection".to_owned(),
    };
    let current = if st.current.is_empty() {
        "—".to_owned()
    } else {
        st.current.join(", ")
    };
    widgets::card(ui, &pal, |ui| {
        widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Globe,
            &conn_text,
            &format!("DNS in use: {current}"),
            Right::None,
        );
    });
    ui.add_space(10.0);

    // A preset is active when its first server is in use; Automatic otherwise.
    let active_id = st
        .presets
        .iter()
        .find(|p| p.servers.first().is_some_and(|s0| st.current.contains(s0)))
        .map(|p| p.id.clone())
        .unwrap_or_else(|| "auto".to_owned());
    let fastest = st
        .presets
        .iter()
        .filter_map(|p| {
            let ms = (*st.latency.get(p.servers.first()?)?)?;
            Some((p.id.clone(), ms))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0);
    let uuid = st.conn.as_ref().map(|c| c.0.clone());
    widgets::card(ui, &pal, |ui| {
        for (n, p) in st.presets.iter().enumerate() {
            if n > 0 {
                widgets::divider(ui, &pal);
            }
            let mut sub = p.desc.clone();
            if !p.servers.is_empty() {
                sub = format!("{sub} · {}", p.servers.join(", "));
            }
            if let Some(first) = p.servers.first() {
                match st.latency.get(first) {
                    Some(Some(ms)) => sub = format!("{sub} · {ms:.0} ms"),
                    Some(None) => sub = format!("{sub} · no reply"),
                    None => {}
                }
            }
            if fastest.as_deref() == Some(p.id.as_str()) {
                sub = format!("{sub} · fastest");
            }
            let right = if p.id == active_id {
                Right::Check
            } else {
                Right::AccentText("Use")
            };
            if widgets::row(ui, &pal, pal.accent, Icon::Globe, &p.title, &sub, right).clicked()
                && p.id != active_id
                && !st.busy
            {
                if let Some(u) = &uuid {
                    let mut args = vec!["dns-set".to_owned(), u.clone()];
                    args.extend(p.servers.iter().cloned());
                    job = Some((args, true));
                }
            }
        }
    });
    ui.add_space(10.0);

    widgets::section_label(ui, &pal, "Custom DNS");
    widgets::card(ui, &pal, |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.add(
                egui::TextEdit::singleline(&mut st.custom)
                    .hint_text("e.g. 1.1.1.2 1.0.0.2")
                    .desired_width(260.0),
            );
            if widgets::ghost_button(ui, &pal, "Use", None).clicked() && !st.busy {
                let servers: Vec<String> = st
                    .custom
                    .split([' ', ','])
                    .filter(|x| !x.is_empty())
                    .map(str::to_owned)
                    .collect();
                if let (Some(u), false) = (&uuid, servers.is_empty()) {
                    let mut args = vec!["dns-set".to_owned(), u.clone()];
                    args.extend(servers);
                    job = Some((args, true));
                }
            }
        });
        ui.add_space(8.0);
    });
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        if widgets::ghost_button(ui, &pal, "Find fastest", Some(Icon::Sparkle)).clicked()
            && !st.busy
        {
            let mut args = vec!["dns-test".to_owned()];
            for p in &st.presets {
                args.extend(p.servers.first().cloned());
            }
            job = Some((args, false));
        }
        if widgets::ghost_button(ui, &pal, "Clear DNS cache", Some(Icon::Trash)).clicked()
            && !st.busy
        {
            job = Some((vec!["dns-flush".to_owned()], false));
        }
        if widgets::ghost_button(ui, &pal, "Refresh", Some(Icon::Update)).clicked() && !st.busy {
            job = Some((vec!["dns-info".to_owned()], true));
        }
        if st.busy {
            ui.spinner();
        }
    });
    faint(
        ui,
        &pal,
        "DNS is set on the active NetworkManager connection (IPv4) and the connection is \
         restarted. Automatic gives it back to your router or provider.",
    );
    let log = st.log.clone();
    drop(st);
    log_card(ui, &pal, &log);
    if let Some((args, reload)) = job {
        if args[0] == "dns-info" {
            load_dns(state, ctx);
        } else {
            dns_job(state, ctx, args, reload);
        }
    }
}
