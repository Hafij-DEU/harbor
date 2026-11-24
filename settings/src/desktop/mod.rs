//! "Desktop & windows" (the Ubuntu Settings and GNOME Tweaks options that the
//! other pages do not cover) and "Extensions". Everything is written straight
//! to gsettings, the autostart folder or the shell's D-Bus API, so neither
//! GNOME Settings nor Tweaks nor the Extensions app is needed. Rows whose
//! schema is not installed (no Ubuntu Dock, no desktop icons) hide themselves.

mod catalog;
mod extensions;
mod fusion;
mod gsettings;
mod startup;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, RichText, Ui};

use crate::app::App;
use crate::pages::{combo_row, range_row};
use crate::theme::regular;
use crate::widgets::{self, Right, ROW_PAD};
pub use catalog::*;
pub use extensions::*;
pub use fusion::*;
pub use gsettings::*;
pub use startup::*;

pub(crate) enum Done {
    Message(Result<String, String>),
    Extensions(Vec<Extension>),
    Found(Result<Vec<Found>, String>),
    Options(String, Vec<Opt>),
    FusionInstalled(Result<String, String>),
}

#[derive(Default)]
pub struct State {
    loaded: Option<Loaded>,
    rx: Option<Receiver<Loaded>>,
    jobs: Vec<Receiver<Done>>,
    extensions: Option<Vec<Extension>>,
    ext_loading: bool,
    user_extensions_on: Option<bool>,
    query: String,
    found: Vec<Found>,
    searching: bool,
    open: std::collections::HashSet<String>,
    options: HashMap<String, Vec<Opt>>,
    fusion_installing: bool,
}

fn section_title(ui: &mut Ui, app: &App, text: &str) {
    let pal = app.pal();
    ui.add_space(18.0);
    widgets::section_label(ui, &pal, text);
}

fn show(app: &mut App, ui: &mut Ui, only: Option<usize>) {
    let pal = app.pal();
    match only {
        Some(i) => widgets::h1(ui, &pal, section_name(i as u8)),
        None => {
            widgets::h1(ui, &pal, "Desktop & windows");
            ui.label(
                RichText::new("Multitasking, dock, windows, fonts and themes — the Ubuntu and Tweaks options in one place.")
                    .font(regular(12.5))
                    .color(pal.text_faint),
            );
        }
    }

    if let Some(rx) = &app.desk.rx {
        if let Ok(loaded) = rx.try_recv() {
            app.desk.loaded = Some(loaded);
            app.desk.rx = None;
        }
    }
    if app.desk.loaded.is_none() {
        if app.desk.rx.is_none() {
            let (tx, rx) = mpsc::channel();
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || {
                let _ = tx.send(load());
                ctx.request_repaint();
            });
            app.desk.rx = Some(rx);
        }
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Reading your desktop settings…").color(pal.text_muted));
        });
        return;
    }

    // Which sections to draw, and the heading over each (None = the page
    // title already names it).
    let mut plan: Vec<(&'static str, Option<&'static str>)> = Vec::new();
    match only {
        None => {
            plan.extend(SECTIONS.iter().map(|s| (*s, Some(*s))));
            plan.extend(FUSION_GROUPS.iter().map(|g| (*g, Some(*g))));
        }
        Some(i) if i < SECTIONS.len() => plan.push((SECTIONS[i], None)),
        // One Fusion Desk group on its own page.
        Some(i) if i > FUSION_INDEX && i <= FUSION_INDEX + FUSION_GROUPS.len() => {
            plan.push((FUSION_GROUPS[i - FUSION_INDEX - 1], None))
        }
        Some(FUSION_INDEX) => plan.extend(
            FUSION_GROUPS
                .iter()
                .map(|g| (*g, Some(g.trim_start_matches("Fusion Desk · ")))),
        ),
        Some(_) => {}
    }
    let mut set: Option<(usize, String)> = None;
    let mut fusion_shown = false;
    // Group pages skip the status card unless Fusion Desk is missing (then
    // it holds the Install button).
    let fusion_missing = !app
        .desk
        .loaded
        .as_ref()
        .is_some_and(|l| l.values.keys().any(|k| k.starts_with(FUSION_SCHEMA)));
    let fusion_card = only.is_none() || only == Some(FUSION_INDEX) || fusion_missing;
    for (section, heading) in plan {
        if section.starts_with("Fusion Desk") && !fusion_shown && fusion_card {
            fusion_shown = true;
            if only.is_none() {
                section_title(ui, app, "Fusion Desk");
            } else {
                ui.add_space(14.0);
            }
            fusion_header(app, ui);
        }
        let rows: Vec<usize> = ITEMS
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                it.section == section
                    && app.desk.loaded.as_ref().is_some_and(|l| {
                        l.values.contains_key(&format!("{} {}", it.schema, it.key))
                    })
            })
            .map(|(i, _)| i)
            .collect();
        if rows.is_empty() {
            continue;
        }
        match heading {
            Some(text) => section_title(ui, app, text),
            None => ui.add_space(14.0),
        }
        let loaded = app.desk.loaded.as_ref().unwrap();
        widgets::card(ui, &pal, |ui| {
            for (n, &i) in rows.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let it = &ITEMS[i];
                let id = format!("{} {}", it.schema, it.key);
                let raw = loaded.values.get(&id).cloned().unwrap_or_default();
                let current = plain(&raw);
                match it.kind {
                    Kind::Bool | Kind::BoolInverted => {
                        let invert = matches!(it.kind, Kind::BoolInverted);
                        let mut on = (current == "true") != invert;
                        if widgets::row(
                            ui,
                            &pal,
                            pal.accent,
                            it.icon,
                            it.title,
                            it.sub,
                            Right::Switch(&mut on),
                        )
                        .toggled()
                        {
                            set = Some((i, ((on != invert) as u8 == 1).to_string()));
                        }
                    }
                    Kind::Choice(options) => {
                        let opts: Vec<(String, String)> = options
                            .iter()
                            .map(|(v, l)| ((*v).to_owned(), (*l).to_owned()))
                            .collect();
                        if let Some(v) = combo_row(
                            ui, &pal, it.icon, it.title, it.sub, &id, &opts, &current, 230.0,
                        ) {
                            set = Some((i, v));
                        }
                    }
                    Kind::Int(min, max, step) => {
                        let mut value = current.parse::<f32>().unwrap_or(min as f32);
                        let sub = format!("{} · {}", it.sub, value as i64);
                        if range_row(
                            ui,
                            &pal,
                            it.icon,
                            it.title,
                            sub,
                            &mut value,
                            min as f32,
                            max as f32,
                            step as f32,
                        ) {
                            set = Some((i, (value as i64).to_string()));
                        }
                    }
                    Kind::Font => {
                        let (family, size) = match current.rsplit_once(' ') {
                            Some((f, s)) if s.parse::<f32>().is_ok() => {
                                (f.to_owned(), s.to_owned())
                            }
                            _ => (current.clone(), "11".to_owned()),
                        };
                        let opts: Vec<(String, String)> = loaded
                            .fonts
                            .iter()
                            .map(|f| (f.clone(), f.clone()))
                            .collect();
                        let sub = format!("{} · {} pt", it.sub, size);
                        if let Some(f) = combo_row(
                            ui, &pal, it.icon, it.title, &sub, &id, &opts, &family, 230.0,
                        ) {
                            set = Some((i, format!("{f} {size}")));
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(ROW_PAD + 42.0);
                            ui.label(
                                RichText::new("Size")
                                    .font(regular(12.0))
                                    .color(pal.text_muted),
                            );
                            let mut s = size.parse::<f32>().unwrap_or(11.0);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut s)
                                        .range(6.0..=36.0)
                                        .speed(0.2)
                                        .fixed_decimals(0),
                                )
                                .changed()
                            {
                                set = Some((i, format!("{family} {}", s.round())));
                            }
                        });
                        ui.add_space(8.0);
                    }
                    Kind::Theme(kind) => {
                        let opts: Vec<(String, String)> = loaded
                            .themes
                            .get(&(kind as u8))
                            .map(|l| l.iter().map(|t| (t.clone(), t.clone())).collect())
                            .unwrap_or_default();
                        if let Some(v) = combo_row(
                            ui, &pal, it.icon, it.title, it.sub, &id, &opts, &current, 230.0,
                        ) {
                            set = Some((i, v));
                        }
                    }
                }
            }
        });
    }
    if let Some((i, value)) = set {
        let it = &ITEMS[i];
        if write(it, &value) {
            let stored = match it.kind {
                Kind::Bool | Kind::BoolInverted | Kind::Int(..) => value,
                _ => format!("'{value}'"),
            };
            if let Some(l) = app.desk.loaded.as_mut() {
                l.values.insert(format!("{} {}", it.schema, it.key), stored);
            }
            app.notify(format!("{} updated", it.title));
        } else {
            app.notify_error(format!("Could not change {}", it.title.to_lowercase()));
        }
    }

    if only.is_none() {
        startup_apps(app, ui, true);
    } else if only == Some(SECTIONS.len()) {
        ui.add_space(14.0);
        startup_apps(app, ui, false);
    }
}
