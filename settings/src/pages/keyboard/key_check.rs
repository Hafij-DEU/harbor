//! "Key check" card: press keys on any keyboard and see which key it was and
//! which keyboard sent it, which keys are held right now (and for how long,
//! so a stuck key shows up), and how often one chosen key was pressed.
//! The service reads the keys (Settings cannot see Caps Lock, media keys or
//! which keyboard a key came from); see `keyboard/status.rs`.

use std::time::{Duration, Instant};

use eframe::egui::{self, Ui};

use super::state;
use crate::app::App;
use crate::icons::Icon;
use crate::keyboard::keys::{self, KEYS};
use crate::keyboard::status::KeyCheck;
use crate::system;
use crate::theme::Palette;
use crate::widgets::{self, Right};

/// How often the request file is touched while the card is open.
const ASK_EVERY: Duration = Duration::from_secs(1);
/// Held this long counts as "looks stuck" in the list.
const LOOKS_STUCK_MS: u64 = 4000;
const SHOW_PRESSES: usize = 12;

/// Key check state on `KeyboardUi`.
#[derive(Default)]
pub struct KeyCheckUi {
    pub on: bool,
    /// Last frame the card was drawn while on (app shortcuts pause then).
    pub shown: Option<Instant>,
    asked: Option<Instant>,
    data: Option<KeyCheck>,
    /// The one key being counted, and its count.
    watch_key: u16,
    watch_count: u32,
    /// Newest press time already counted.
    seen_until: u64,
}

impl KeyCheckUi {
    /// Key check is open on screen: keys belong to it, not to app shortcuts.
    pub fn active(&self) -> bool {
        self.on
            && self
                .shown
                .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
    }
}

pub fn show(app: &mut App, ui: &mut Ui, pal: &Palette) {
    widgets::section_label(ui, pal, "Key check");
    let running = app.keyboard.service_running;
    let mut start_service = false;
    widgets::card(ui, pal, |ui| {
        let kc = &mut app.keyboard.key_check;
        let was_on = kc.on;
        widgets::row(
            ui,
            pal,
            pal.accent,
            Icon::Keyboard,
            "Check keys",
            "Press keys on any keyboard: see the key, the keyboard it came from, and keys held down right now",
            Right::Switch(&mut kc.on),
        );
        if was_on && !kc.on {
            system::stop_keycheck();
            kc.data = None;
            kc.asked = None;
        }
        if !kc.on {
            return;
        }
        kc.shown = Some(Instant::now());
        widgets::divider(ui, pal);
        ui.add_space(8.0);
        if !running {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new("Key check needs the keyboard service.")
                        .size(12.5)
                        .color(pal.text_muted),
                );
                if ui
                    .link(egui::RichText::new("Start it").color(pal.accent))
                    .clicked()
                {
                    start_service = true;
                }
            });
            ui.add_space(6.0);
            return;
        }
        if kc.asked.is_none_or(|t| t.elapsed() >= ASK_EVERY) {
            kc.asked = Some(Instant::now());
            system::request_keycheck();
        }
        kc.data = KeyCheck::read();
        count_watched(kc);
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        let names = |id: &str| app_name(&app.keyboard.boards, id);
        // Same left inset as the row text above.
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.vertical(|ui| draw(ui, pal, kc, &names));
        });
    });
    if start_service {
        let cfg = app.keyboard.saved().clone();
        state::save(app, cfg, "Keyboard service started");
    }
}

fn app_name(boards: &[crate::keyboard::devices::Keyboard], id: &str) -> String {
    match boards.iter().find(|k| k.id == id) {
        Some(k) if k.is_internal() => "Laptop keyboard".to_owned(),
        Some(k) => k.name.clone(),
        None => format!("Keyboard {id}"),
    }
}

/// Add new presses of the watched key to its count.
fn count_watched(kc: &mut KeyCheckUi) {
    let Some(data) = &kc.data else { return };
    for (at, _, code) in &data.presses {
        if *at > kc.seen_until {
            if *code == kc.watch_key {
                kc.watch_count += 1;
            }
            kc.seen_until = *at;
        }
    }
}

fn draw(ui: &mut Ui, pal: &Palette, kc: &mut KeyCheckUi, name: &dyn Fn(&str) -> String) {
    let Some(data) = kc.data.clone() else {
        ui.label(
            egui::RichText::new("Waiting for the keyboard service…")
                .size(12.5)
                .color(pal.text_muted),
        );
        ui.add_space(6.0);
        return;
    };
    let faint = |t: &str| {
        egui::RichText::new(t.to_owned())
            .size(12.5)
            .color(pal.text_muted)
    };

    // Last key, big.
    match data.presses.last() {
        Some((_, id, code)) => {
            ui.label(
                egui::RichText::new(keys::any_label(*code))
                    .size(26.0)
                    .color(super::colour::KEY_CHECK),
            );
            ui.label(faint(&format!("from {} · key code {code}", name(id))));
        }
        None => {
            ui.label(
                egui::RichText::new("Press any key")
                    .size(20.0)
                    .color(pal.text_muted),
            );
        }
    }
    ui.add_space(8.0);

    // Held now.
    ui.label(
        egui::RichText::new("Held down now")
            .size(14.0)
            .color(pal.text),
    );
    if data.held.is_empty() {
        ui.label(faint("Nothing"));
    }
    for (id, code, ms) in &data.held {
        let secs = *ms as f32 / 1000.0;
        let stuck = *ms >= LOOKS_STUCK_MS;
        let text = format!(
            "{} on {} · {secs:.1} s{}",
            keys::any_label(*code),
            name(id),
            if stuck { " · looks stuck" } else { "" }
        );
        let color = if stuck { super::WARNING } else { pal.text };
        ui.label(egui::RichText::new(text).size(13.0).color(color));
    }
    ui.add_space(8.0);

    // Recent presses, newest first.
    ui.label(
        egui::RichText::new("Recent keys")
            .size(14.0)
            .color(pal.text),
    );
    ui.horizontal_wrapped(|ui| {
        for (_, id, code) in data.presses.iter().rev().take(SHOW_PRESSES) {
            super::keycap(ui, &keys::any_label(*code), super::colour::FROM_KEY);
            ui.label(faint(&name(id))).on_hover_text("Keyboard that sent it");
        }
    });
    ui.add_space(8.0);

    // One chosen key.
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Count one key")
                .size(14.0)
                .color(pal.text),
        );
        let before = kc.watch_key;
        egui::ComboBox::from_id_salt("kb-keycheck-watch")
            .selected_text(if kc.watch_key == 0 {
                "Choose a key"
            } else {
                keys::label(kc.watch_key)
            })
            .width(170.0)
            .height(320.0)
            .show_ui(ui, |ui| {
                for (_, code, label) in KEYS.iter().filter(|k| k.1 != keys::DISABLED) {
                    ui.selectable_value(&mut kc.watch_key, *code, *label);
                }
            });
        if kc.watch_key != before {
            kc.watch_count = 0;
        }
        if kc.watch_key != 0 {
            let held = data.held.iter().any(|(_, c, _)| *c == kc.watch_key);
            ui.label(faint(&format!(
                "pressed {} time(s) · {}",
                kc.watch_count,
                if held { "held now" } else { "up" }
            )));
            if ui
                .link(egui::RichText::new("Reset").color(pal.accent))
                .clicked()
            {
                kc.watch_count = 0;
            }
        }
    });
    ui.add_space(6.0);
}
