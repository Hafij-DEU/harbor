//! "Keyboards" card: every keyboard with an On switch and what to do when one
//! of its keys gets stuck, plus the default for all keyboards.

use eframe::egui::{self, Ui};

use super::state;
use crate::app::App;
use crate::icons::Icon;
use crate::keyboard::config::{self, Config, KeyFilter, StuckAction, StuckTime};
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
        // Plugged-in keyboards, then unplugged ones that have settings.
        let mut targets: Vec<Target> = app.keyboard.boards.iter().map(|k| k.target()).collect();
        targets.dedup();
        for b in &saved.boards {
            if !targets.contains(&b.target) && (b.off || b.stuck.is_some()) {
                targets.push(b.target.clone());
            }
        }
        for (i, target) in targets.iter().enumerate() {
            if i > 0 {
                widgets::divider(ui, pal);
            }
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

    // Stuck-key guard: the default action up front, the rest under Advanced.
    widgets::section_label(ui, pal, "Stuck keys");
    let mut open = app.keyboard.guard_advanced;
    widgets::card(ui, pal, |ui| {
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
        widgets::divider(ui, pal);
        let sub = if open {
            "Hide keys and times".to_owned()
        } else {
            guard_summary(&saved)
        };
        let row = widgets::row(
            ui,
            pal,
            pal.accent,
            Icon::Keyboard,
            "Advanced",
            &sub,
            Right::Chevron,
        );
        if row.clicked() {
            open = !open;
        }
        if open {
            if let Some(cfg) = key_filter(ui, pal, &saved, busy) {
                change = Some((cfg, "Stuck-key filter saved"));
            }
            if let Some(cfg) = stuck_times(ui, pal, &saved, busy) {
                change = Some((cfg, "Stuck-key times saved"));
            }
        }
        ui.add_space(6.0);
    });
    app.keyboard.guard_advanced = open;

    let t = saved.stuck_time;
    let disable = if t.again == 0 {
        "A key is never disabled for sticking again: the keyboard is only restarted.".to_owned()
    } else {
        format!(
            "If the key sticks again within {}, it is disabled until you turn it back on here.",
            config::seconds_label(t.again)
        )
    };
    super::super::note(
        ui,
        pal,
        &format!(
            "Restart resets the keyboard so it lets go of the key. {disable} A key counts as \
             stuck when held {} on the laptop keyboard or {} on others while the keyboard \
             sends nothing. Pick the keys and times under Advanced.",
            config::seconds_label(t.laptop),
            config::seconds_label(t.other)
        ),
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

/// One line for the closed Advanced row: "All keys · Laptop 4 s · Others 30 s · Disable within 1 min".
fn guard_summary(cfg: &Config) -> String {
    let names = |k: &[u16]| match k.len() {
        0 => "none".to_owned(),
        1..=2 => k.iter().map(|&c| keys::any_label(c)).collect::<Vec<_>>().join(", "),
        n => format!("{n} keys"),
    };
    let keys = match &cfg.stuck_keys {
        KeyFilter::All => "All keys".to_owned(),
        KeyFilter::Except(k) => format!("All keys except {}", names(k)),
        KeyFilter::Only(k) => format!("Only {}", names(k)),
    };
    let t = cfg.stuck_time;
    let disable = if t.again == 0 {
        "Never disable".to_owned()
    } else {
        format!("Disable within {}", config::seconds_label(t.again))
    };
    format!(
        "{keys} · Laptop {} · Others {} · {disable}",
        config::seconds_label(t.laptop),
        config::seconds_label(t.other)
    )
}

/// Three time rows: laptop limit, other-keyboard limit, disable window.
/// Returns the config to save.
fn stuck_times(ui: &mut Ui, pal: &Palette, saved: &Config, busy: bool) -> Option<Config> {
    let t = saved.stuck_time;
    let choices = |list: &[u16], current: u16, fmt: &dyn Fn(u16) -> String| {
        let mut list = list.to_vec();
        if !list.contains(&current) {
            list.push(current);
            list.sort_unstable();
        }
        list.into_iter()
            .map(|s| (s.to_string(), fmt(s)))
            .collect::<Vec<_>>()
    };
    let held = |s: u16| format!("Held {}", config::seconds_label(s));
    let mut next = t;
    let rows: [(&str, &str, &str, Vec<(String, String)>, u16, u8); 3] = [
        (
            "Laptop keyboard: stuck after",
            "A key held this long while the keyboard sends nothing",
            "kb-stuck-laptop",
            choices(&StuckTime::LAPTOP_CHOICES, t.laptop, &held),
            t.laptop,
            0,
        ),
        (
            "Other keyboards: stuck after",
            "USB and Bluetooth keyboards stay silent while a key is held",
            "kb-stuck-other",
            choices(&StuckTime::OTHER_CHOICES, t.other, &held),
            t.other,
            1,
        ),
        (
            "Disable the key if it sticks again within",
            "After a restart; Never only restarts the keyboard",
            "kb-stuck-again",
            choices(&StuckTime::AGAIN_CHOICES, t.again, &config::seconds_label),
            t.again,
            2,
        ),
    ];
    for (title, sub, id, options, current, which) in &rows {
        widgets::divider(ui, pal);
        let picked = super::super::combo_row(
            ui,
            pal,
            Icon::Keyboard,
            title,
            sub,
            id,
            options,
            &current.to_string(),
            190.0,
        );
        if let Some(v) = picked.and_then(|p| p.parse::<u16>().ok()) {
            match which {
                0 => next.laptop = v,
                1 => next.other = v,
                _ => next.again = v,
            }
        }
    }
    if next == t || busy {
        return None;
    }
    let mut cfg = saved.clone();
    cfg.stuck_time = next;
    Some(cfg)
}

/// Left and right Ctrl, Shift, Alt and Super.
const MODIFIER_KEYS: [u16; 8] = [29, 97, 42, 54, 56, 100, 125, 126];

/// "Keys that count" row: All / All except these / Only these, then the key
/// list with remove buttons and an Add key picker. Returns the config to save.
fn key_filter(ui: &mut Ui, pal: &Palette, saved: &Config, busy: bool) -> Option<Config> {
    let filter = &saved.stuck_keys;
    let modes: Vec<(String, String)> = KeyFilter::MODES
        .iter()
        .map(|(w, l)| ((*w).to_owned(), (*l).to_owned()))
        .collect();
    let mut next: Option<KeyFilter> = None;
    widgets::divider(ui, pal);
    let picked = super::super::combo_row(
        ui,
        pal,
        Icon::Keyboard,
        "Keys that count",
        "Which stuck keys restart a keyboard or get disabled",
        "kb-stuck-keys",
        &modes,
        filter.word(),
        190.0,
    );
    if let Some(word) = picked {
        next = Some(filter.with_mode(&word));
    }
    if *filter != KeyFilter::All {
        let listed = filter.keys().to_vec();
        ui.horizontal_wrapped(|ui| {
            ui.add_space(52.0);
            if listed.is_empty() {
                ui.label(
                    egui::RichText::new(match filter {
                        KeyFilter::Only(_) => "No keys yet: the guard does nothing.",
                        _ => "No keys left out yet.",
                    })
                    .size(12.5)
                    .color(pal.text_muted),
                );
            }
            for &code in &listed {
                let chip = ui
                    .button(egui::RichText::new(format!("{}  ✕", keys::any_label(code))).size(12.5))
                    .on_hover_text("Remove");
                if chip.clicked() {
                    let keep = listed.iter().copied().filter(|&c| c != code).collect();
                    next = Some(filter.with_keys(keep));
                }
            }
            egui::ComboBox::from_id_salt("kb-stuck-keys-add")
                .selected_text("Add key")
                .width(150.0)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for &(_, code, label) in keys::KEYS {
                        if code == keys::DISABLED || listed.contains(&code) {
                            continue;
                        }
                        if ui.selectable_label(false, label).clicked() {
                            let mut all = listed.clone();
                            all.push(code);
                            next = Some(filter.with_keys(all));
                        }
                    }
                });
            if MODIFIER_KEYS.iter().any(|c| !listed.contains(c))
                && ui
                    .link(egui::RichText::new("Add Ctrl, Shift, Alt, Super").color(pal.accent))
                    .clicked()
            {
                let mut all = listed.clone();
                all.extend(MODIFIER_KEYS);
                next = Some(filter.with_keys(all));
            }
        });
        ui.add_space(4.0);
    }
    let next = next.filter(|f| f != filter && !busy)?;
    let mut cfg = saved.clone();
    cfg.stuck_keys = next;
    Some(cfg)
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
