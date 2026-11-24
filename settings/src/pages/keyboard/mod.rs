//! Bluetooth & devices > Keyboard.
//!
//! Top: Keyboard Manager, one file per card:
//! * `keyboards.rs` - each keyboard on/off and what to do when a key sticks;
//! * `key_check.rs` - live view of the keys each keyboard presses or holds;
//! * `remaps.rs` - remap keys and shortcuts (PowerToys style).
//!
//! `state.rs` holds what these cards remember between frames and saves the
//! config. Below them: input sources and key repeat (this file).
//! How the whole feature works: `src/keyboard/mod.rs`.

mod key_check;
mod keyboards;
mod remaps;
mod state;

pub use state::KeyboardUi;

/// Amber for stuck or disabled keys (readable on light and dark cards).
const WARNING: egui::Color32 = egui::Color32::from_rgb(0xd0, 0x80, 0x00);

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::system;
use crate::widgets::{self, Right};

use super::*;

pub fn keyboard(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Keyboard") {
        app.back();
        return;
    }

    spacer(ui, 16.0);
    state::refresh(app, ui);
    keyboards::show(app, ui, &pal);
    spacer(ui, 20.0);
    key_check::show(app, ui, &pal);
    spacer(ui, 20.0);
    remaps::show(app, ui, &pal);
    spacer(ui, 20.0);
    widgets::section_label(ui, &pal, "Input sources");
    // Cycle layout with Super+Space / Super+Shift+Space, like GNOME.
    let mut remove: Option<String> = None;
    let mut add: Option<String> = None;
    let mut filter = app.keyboard_filter.clone();
    widgets::card(ui, &pal, |ui| {
        let sources = app.info.input_sources.clone();
        if sources.is_empty() {
            widgets::empty_note(ui, &pal, "No input sources reported.");
        }
        for (index, id) in sources.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let name = system::KB_LAYOUTS
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, name)| *name)
                .unwrap_or(id.as_str());
            let can_remove = sources.len() > 1;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Keyboard,
                name,
                id,
                if can_remove {
                    Right::AccentText("Remove")
                } else {
                    Right::None
                },
            );
            if can_remove && outcome.clicked() {
                remove = Some(id.clone());
            }
        }
        widgets::divider(ui, &pal);
        _ = widgets::search_box(ui, &pal, &mut filter, false, "Search layouts, e.g. German");
        let query = filter.trim().to_lowercase();
        let mut shown = 0;
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .id_salt("kb-layouts")
            .show(ui, |ui| {
                for (id, name) in system::KB_LAYOUTS {
                    if sources.iter().any(|s| s == id) {
                        continue;
                    }
                    let hay = format!("{id} {name}").to_lowercase();
                    if !query.is_empty() && !hay.contains(&query) {
                        continue;
                    }
                    shown += 1;
                    if widgets::row(
                        ui,
                        &pal,
                        pal.accent,
                        Icon::Keyboard,
                        name,
                        id,
                        Right::AccentText("Add"),
                    )
                    .clicked()
                    {
                        add = Some((*id).to_owned());
                    }
                }
                if shown == 0 {
                    widgets::empty_note(
                        ui,
                        &pal,
                        if query.is_empty() {
                            "Every listed layout is already added."
                        } else {
                            "No layout matches that filter."
                        },
                    );
                }
            });
    });
    if filter != app.keyboard_filter {
        app.keyboard_filter = filter;
    }
    if let Some(id) = remove {
        let remaining: Vec<String> = app
            .info
            .input_sources
            .iter()
            .filter(|s| **s != id)
            .cloned()
            .collect();
        if system::set_input_sources(&remaining) {
            app.info.input_sources = remaining;
            app.notify("Input source removed");
        } else {
            app.notify_error("Could not change the input sources");
        }
    }
    if let Some(id) = add {
        let mut next = app.info.input_sources.clone();
        if !next.contains(&id) {
            next.push(id);
        }
        if system::set_input_sources(&next) {
            app.info.input_sources = next;
            app.keyboard_filter.clear();
            app.notify("Input source added");
        } else {
            app.notify_error("Could not change the input sources");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Key repeat");
    widgets::card(ui, &pal, |ui| {
        let mut repeat = app.info.key_repeat;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Keyboard,
            "Key repeat",
            "Hold a key to repeat it",
            Right::Switch(&mut repeat),
        );
        if outcome.toggled {
            if system::set_key_repeat(repeat) {
                app.info.key_repeat = repeat;
            } else {
                app.notify_error("Could not change key repeat");
            }
        }
        widgets::divider(ui, &pal);
        let mut delay = ((app.info.repeat_delay as f32 - 100.0) / 900.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Keyboard,
            "Delay",
            format!("{} ms before repeat starts", app.info.repeat_delay),
            &mut delay,
        );
        if changed {
            let ms = (100.0 + delay * 900.0).round() as u32;
            if system::set_repeat_delay(ms) {
                app.info.repeat_delay = ms;
            } else {
                app.notify_error("Could not change the repeat delay");
            }
        }
        widgets::divider(ui, &pal);
        let mut interval = ((app.info.repeat_interval as f32 - 10.0) / 110.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Keyboard,
            "Repeat rate",
            format!("{} ms between repeats", app.info.repeat_interval),
            &mut interval,
        );
        if changed {
            let ms = (10.0 + interval * 110.0).round() as u32;
            if system::set_repeat_interval(ms) {
                app.info.repeat_interval = ms;
            } else {
                app.notify_error("Could not change the repeat rate");
            }
        }
    });
    note(ui, &pal, "Cycle input sources with Super+Space.");
}
