//! "Keyboards" card: every keyboard with an On switch and what to do when one
//! of its keys gets stuck, plus the default for all keyboards.

use eframe::egui::{self, Ui};

use super::state;
use crate::app::App;
use crate::icons::Icon;
use crate::keyboard::config::StuckAction;
use crate::keyboard::devices::Target;
use crate::keyboard::keys;
use crate::system;
use crate::theme::Palette;
use crate::widgets::{self, Right};

pub fn show(app: &mut App, ui: &mut Ui, pal: &Palette) {
    widgets::section_label(ui, pal, "Keyboards");
    let saved = app.keyboard.saved().clone();
    let busy = app.keyboard.busy();
    let mut change: Option<(crate::keyboard::config::Config, &'static str)> = None;
    let mut restart = false;
    let mut stop_watchdog = false;

    widgets::card(ui, pal, |ui| {
        // Default stuck-key action.
        let options: Vec<(String, String)> = StuckAction::ALL
            .iter()
            .map(|a| (a.word().to_owned(), a.label().to_owned()))
            .collect();
        let picked = super::super::combo_row(
            ui,
            pal,
            Icon::Keyboard,
            "When a key gets stuck",
            "Default for every keyboard",
            "kb-stuck-default",
            &options,
            saved.stuck.word(),
            190.0,
        );
        if let Some(a) = picked.as_deref().and_then(StuckAction::parse) {
            let mut cfg = saved.clone();
            cfg.stuck = a;
            change = Some((cfg, "Stuck-key setting saved"));
        }

        // Plugged-in keyboards, then unplugged ones that have settings.
        let mut targets: Vec<Target> = app.keyboard.boards.iter().map(|k| k.target()).collect();
        targets.dedup();
        for b in &saved.boards {
            if !targets.contains(&b.target) && (b.off || b.stuck.is_some()) {
                targets.push(b.target.clone());
            }
        }
        for (i, target) in targets.iter().enumerate() {
            widgets::divider(ui, pal);
            let board = saved.board(target);
            let mut on = !board.is_some_and(|b| b.off);
            let plugged = app.keyboard.boards.iter().find(|k| &k.target() == target);
            let title = match (target, plugged) {
                (Target::Internal, _) => "Laptop keyboard".to_owned(),
                (_, Some(k)) => k.name.clone(),
                _ => app.keyboard.board_name(target),
            };
            let sub = board_subtitle(app, target, plugged.map(|k| k.name.as_str()), on);
            let row = widgets::row(
                ui,
                pal,
                pal.accent,
                Icon::Keyboard,
                &title,
                &sub,
                Right::Switch(&mut on),
            );
            if row.toggled && !busy {
                let mut cfg = saved.clone();
                cfg.board_mut(target).off = !on;
                change = Some((
                    cfg,
                    if on {
                        "Keyboard turned on"
                    } else {
                        "Keyboard turned off"
                    },
                ));
            }

            // Own stuck-key action.
            let mut options = vec![(
                "default".to_owned(),
                format!("Same as all ({})", saved.stuck.label()),
            )];
            options.extend(
                StuckAction::ALL
                    .iter()
                    .map(|a| (a.word().to_owned(), a.label().to_owned())),
            );
            let current = board.and_then(|b| b.stuck).map_or("default", |a| a.word());
            ui.horizontal(|ui| {
                ui.add_space(52.0);
                ui.label(
                    egui::RichText::new("When a key sticks")
                        .size(12.5)
                        .color(pal.text_muted),
                );
                egui::ComboBox::from_id_salt(("kb-stuck", i))
                    .selected_text(
                        options
                            .iter()
                            .find(|o| o.0 == current)
                            .map_or("", |o| o.1.as_str()),
                    )
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (value, label) in &options {
                            if ui.selectable_label(value == current, label).clicked()
                                && value != current
                                && !busy
                            {
                                let mut cfg = saved.clone();
                                cfg.board_mut(target).stuck = StuckAction::parse(value);
                                change = Some((cfg, "Stuck-key setting saved"));
                            }
                        }
                    });
            });

            // Keys the service disabled after they stuck.
            let disabled = disabled_keys(app, target);
            if !disabled.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(52.0);
                    ui.label(
                        egui::RichText::new(format!("Disabled after sticking: {disabled}"))
                            .size(12.5)
                            .color(super::WARNING),
                    );
                    if ui
                        .link(egui::RichText::new("Turn back on").color(pal.accent))
                        .clicked()
                    {
                        restart = true;
                    }
                });
            }
            ui.add_space(4.0);
        }

        // Service and Toggle Internal Keyboard notes.
        if !app.keyboard.service_running {
            widgets::divider(ui, pal);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new("The keyboard service is not running.")
                        .size(12.5)
                        .color(pal.text_muted),
                );
                if ui
                    .link(egui::RichText::new("Start it").color(pal.accent))
                    .clicked()
                {
                    change = Some((saved.clone(), "Keyboard service started"));
                }
            });
        }
        let guarding = saved.stuck != StuckAction::Nothing
            || saved
                .board(&Target::Internal)
                .and_then(|b| b.stuck)
                .is_some_and(|a| a != StuckAction::Nothing);
        if app.keyboard.toggle_watchdog && guarding {
            widgets::divider(ui, pal);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(
                        "Toggle Internal Keyboard's own stuck-Shift watchdog is also running. \
                         Two watchdogs may both restart the laptop keyboard.",
                    )
                    .size(12.5)
                    .color(pal.text_muted),
                );
                if ui
                    .link(egui::RichText::new("Turn its watchdog off").color(pal.accent))
                    .clicked()
                {
                    stop_watchdog = true;
                }
            });
        }
        ui.add_space(6.0);
    });

    super::super::note(
        ui,
        pal,
        "Restart resets the keyboard so it lets go of the key. If the key sticks again \
         within a minute, it is disabled until you turn it back on here. The laptop \
         keyboard counts as stuck after 4 silent seconds, other keyboards after 30.",
    );

    if let Some((cfg, done)) = change {
        state::save(app, cfg, done);
    } else if restart {
        state::run(
            app,
            None,
            "Disabled keys turned back on",
            system::restart_keyboard_service,
        );
    } else if stop_watchdog {
        state::run(
            app,
            None,
            "Toggle Internal Keyboard's watchdog turned off",
            system::stop_toggle_watchdog,
        );
    }
}

/// "Built in · Off", "USB 046d:c31c · On", "Unplugged · Off"...
fn board_subtitle(app: &App, target: &Target, name: Option<&str>, on: bool) -> String {
    let kind = match (target, name) {
        (Target::Internal, Some(n)) => format!("Built in · {n}"),
        (Target::Internal, None) => "Built in".to_owned(),
        (Target::Id(id), Some(_)) => id.clone(),
        (_, None) => "Unplugged".to_owned(),
        (Target::Every, _) => String::new(),
    };
    let kept_on = app
        .keyboard
        .status
        .as_ref()
        .and_then(|s| {
            s.boards
                .iter()
                .find(|b| board_matches(target, &b.id, b.bus))
        })
        .is_some_and(|b| b.kept_on);
    let state = if kept_on {
        "Set to off, kept on: it is the only keyboard"
    } else if on {
        "On"
    } else {
        "Off: its keys do nothing"
    };
    format!("{kind} · {state}")
}

fn board_matches(target: &Target, id: &str, bus: u16) -> bool {
    *target != Target::Every && target.matches(id, bus)
}

/// Labels of keys the service disabled on this keyboard, comma-separated.
fn disabled_keys(app: &App, target: &Target) -> String {
    let Some(status) = &app.keyboard.status else {
        return String::new();
    };
    status
        .boards
        .iter()
        .filter(|b| board_matches(target, &b.id, b.bus))
        .flat_map(|b| b.disabled.iter().map(|&c| keys::any_label(c)))
        .collect::<Vec<_>>()
        .join(", ")
}
