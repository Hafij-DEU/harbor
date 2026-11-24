//! System > Power & battery: profiles, sleep, lid, power button, battery saver.

use eframe::egui::{self, Rect, Sense, Ui, Vec2};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::{regular, semibold};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn power(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Power & battery") {
        app.back();
        return;
    }

    widgets::card(ui, &pal, |ui| {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Battery,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let big = match app.info.battery {
                    Some(pct) => format!("{}%", pct),
                    None => "No battery".to_owned(),
                };
                ui.label(
                    egui::RichText::new(big)
                        .font(semibold(22.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(if app.info.battery.is_some() {
                        if app.info.charging {
                            "Charging"
                        } else {
                            "On battery power"
                        }
                    } else {
                        "This device runs on AC power"
                    })
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let width = ui.available_width() - ROW_PAD;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
            let fraction = app.info.battery.unwrap_or(100) as f32 / 100.0;
            ui.painter().rect_filled(rect, pal.corner(4), pal.track);
            let filled = Rect::from_min_size(
                rect.min,
                Vec2::new((rect.width() * fraction).max(6.0), rect.height()),
            );
            ui.painter().rect_filled(filled, pal.corner(4), pal.accent);
        });
        ui.add_space(16.0);
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Power mode");
    let modes = [
        (
            "power-saver",
            "Power saver",
            "Reduce performance to save battery",
        ),
        (
            "balanced",
            "Balanced",
            "Automatic balance of power and performance",
        ),
        (
            "performance",
            "Performance",
            "Favour performance over battery life",
        ),
    ];
    let mut chosen = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (key, label, subtitle)) in modes.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let selected = app.info.power_profile == *key;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Power,
                label,
                subtitle,
                if selected { Right::Check } else { Right::None },
            )
            .clicked()
                && !selected
            {
                chosen = Some(*key);
            }
        }
    });
    if let Some(key) = chosen {
        if system::set_power_profile(key) {
            app.info.power_profile = key.to_owned();
            app.notify(format!(
                "Power mode set to {}",
                capitalize(&key.replace('-', " "))
            ));
        } else {
            app.notify_error("powerprofilesctl is not available");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Screen and sleep");
    let options = [
        (300u32, "5 minutes"),
        (600, "10 minutes"),
        (900, "15 minutes"),
        (1800, "30 minutes"),
    ];
    let mut chosen = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (seconds, label)) in options.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let selected = app.info.idle_delay == *seconds;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Clock,
                label,
                "Turn the screen off after this idle period",
                if selected { Right::Check } else { Right::None },
            )
            .clicked()
                && !selected
            {
                chosen = Some(*seconds);
            }
        }
    });
    if let Some(seconds) = chosen {
        if system::set_idle_delay(seconds) {
            app.info.idle_delay = seconds;
            app.notify("Screen timeout updated");
        } else {
            app.notify_error("Could not change the screen timeout");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Automatic suspend");
    widgets::card(ui, &pal, |ui| {
        let mut on_ac = app.info.sleep_ac;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Power,
            "Suspend on AC power",
            "Sleep automatically when idle on wall power",
            Right::Switch(&mut on_ac),
        );
        if outcome.toggled {
            if system::set_sleep_idle(on_ac, app.info.sleep_battery) {
                app.info.sleep_ac = on_ac;
                app.notify(if on_ac {
                    "Automatic suspend on AC enabled"
                } else {
                    "Automatic suspend on AC disabled"
                });
            } else {
                app.notify_error("Could not change automatic suspend");
            }
        }
        widgets::divider(ui, &pal);
        let mut on_battery = app.info.sleep_battery;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Power,
            "Suspend on battery",
            "Sleep automatically when idle on battery",
            Right::Switch(&mut on_battery),
        );
        if outcome.toggled {
            if system::set_sleep_idle(app.info.sleep_ac, on_battery) {
                app.info.sleep_battery = on_battery;
                app.notify(if on_battery {
                    "Automatic suspend on battery enabled"
                } else {
                    "Automatic suspend on battery disabled"
                });
            } else {
                app.notify_error("Could not change automatic suspend");
            }
        }
    });
    note(
        ui,
        &pal,
        "Uses GNOME's automatic suspend settings; the delay is unchanged.",
    );

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "When the lid is closed");
    let lid_options = [("suspend", "Suspend"), ("nothing", "Do nothing")];
    let mut lid_change = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (title, key)) in [("On AC power", "ac"), ("On battery", "battery")]
            .into_iter()
            .enumerate()
        {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.label(
                    egui::RichText::new(title)
                        .font(semibold(12.0))
                        .color(pal.text_muted),
                );
            });
            for (value, label) in lid_options {
                let current = if key == "ac" {
                    &app.info.lid_ac
                } else {
                    &app.info.lid_battery
                };
                let selected = current == value;
                if widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Power,
                    label,
                    if key == "ac" {
                        "When the lid is closed on wall power"
                    } else {
                        "When the lid is closed on battery"
                    },
                    if selected { Right::Check } else { Right::None },
                )
                .clicked()
                    && !selected
                {
                    lid_change = Some((key.to_owned(), value.to_owned()));
                }
            }
        }
    });
    if let Some((kind, value)) = lid_change {
        let (ac, battery) = if kind == "ac" {
            (value.clone(), app.info.lid_battery.clone())
        } else {
            (app.info.lid_ac.clone(), value.clone())
        };
        if system::set_lid_action(&ac, &battery) {
            app.info.lid_ac = ac;
            app.info.lid_battery = battery;
            app.notify("Lid action updated");
        } else {
            app.notify_error("Could not change the lid action");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Power button");
    let power_options = [
        ("suspend", "Sleep"),
        ("hibernate", "Hibernate"),
        ("interactive", "Ask what to do"),
        ("nothing", "Do nothing"),
    ];
    let power_choices: Vec<(String, String)> = power_options
        .iter()
        .map(|(value, label)| ((*value).to_owned(), (*label).to_owned()))
        .collect();
    if let Some(action) = combo_row(
        ui,
        &pal,
        Icon::Power,
        "When I press the power button",
        "Applies to the hardware power button",
        "power-button-action",
        &power_choices,
        &app.info.power_button,
        180.0,
    ) {
        if system::set_power_button(&action) {
            app.info.power_button = action;
            app.notify("Power button action updated");
        } else {
            app.notify_error("Could not change the power button action");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Battery");
    widgets::card(ui, &pal, |ui| {
        let mut saver = app.info.low_battery_saver;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Battery,
            "Power saver at low battery",
            "Switch to the power-saving profile automatically",
            Right::Switch(&mut saver),
        );
        if outcome.toggled {
            if system::set_low_battery_saver(saver) {
                app.info.low_battery_saver = saver;
                app.notify(if saver {
                    "Low-battery power saver enabled"
                } else {
                    "Low-battery power saver disabled"
                });
            } else {
                app.notify_error("Could not change the low-battery setting");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Top bar");
    widgets::card(ui, &pal, |ui| {
        let mut pct = app.info.battery_percentage;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Battery,
            "Battery percentage",
            "Show the percentage next to the battery icon",
            Right::Switch(&mut pct),
        );
        if outcome.toggled {
            if system::set_clock_detail("show-battery-percentage", pct) {
                app.info.battery_percentage = pct;
            } else {
                app.notify_error("Could not change the battery percentage");
            }
        }
    });
}
