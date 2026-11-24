//! Accessibility: text size, animations, sticky/mouse keys, alerts.

use eframe::egui::{self, Id, Sense, Ui, Vec2};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::regular;
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn accessibility(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Accessibility");

    let mut apply_scale: Option<f32> = None;

    widgets::card(ui, &pal, |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Accessibility,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    egui::RichText::new("Text size")
                        .font(regular(14.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{:.0}% · applies to this app and Ubuntu apps",
                        app.text_scale * 100.0
                    ))
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD + 46.0);
            let width = ui.available_width() - ROW_PAD - 6.0;
            ui.allocate_ui_with_layout(
                Vec2::new(width, 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(width);
                    let mut value = ((app.text_scale - 1.0) / 0.5).clamp(0.0, 1.0);
                    if widgets::slider(ui, &pal, pal.accent, &mut value, Id::new("text-scale")) {
                        let scale = 1.0 + value * 0.5;
                        app.text_scale = scale;
                        apply_scale = Some(scale);
                    }
                },
            );
        });
        ui.add_space(14.0);
    });

    if let Some(scale) = apply_scale {
        ui.ctx().set_zoom_factor(scale * crate::app::APP_ZOOM);
        if !system::set_text_scale(scale) {
            app.notify_error("Could not change the Ubuntu text scale");
        }
        app.a11y.text_scale = scale;
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Vision");
    widgets::card(ui, &pal, |ui| {
        let mut large = app.a11y.large_text || app.text_scale >= 1.15;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Display,
            "Large text",
            "Scale every interface up to 125%",
            Right::Switch(&mut large),
        );
        if outcome.toggled() {
            let scale = if large { 1.25 } else { 1.0 };
            app.a11y.large_text = large;
            app.text_scale = scale;
            ui.ctx().set_zoom_factor(scale * crate::app::APP_ZOOM);
            if system::set_text_scale(scale) {
                app.notify(if large {
                    "Large text turned on"
                } else {
                    "Large text turned off"
                });
            } else {
                app.notify_error("Could not change the text scale");
            }
        }
        widgets::divider(ui, &pal);
        let mut alerts = app.a11y.visual_alerts;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bell,
            "Visual alerts",
            "Flash the screen instead of playing a system beep",
            Right::Switch(&mut alerts),
        );
        if outcome.toggled() {
            if system::set_visual_alerts(alerts) {
                app.a11y.visual_alerts = alerts;
                app.notify("Visual alerts updated");
            } else {
                app.notify_error("Could not change visual alerts");
            }
        }
        widgets::divider(ui, &pal);
        let mut animations = app.a11y.animations;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sparkle,
            "Animations",
            "Turn off for a snappier, calmer interface",
            Right::Switch(&mut animations),
        );
        if outcome.toggled() {
            if system::set_animations(animations) {
                app.a11y.animations = animations;
                app.notify(if animations {
                    "Animations enabled"
                } else {
                    "Animations disabled"
                });
            } else {
                app.notify_error("Could not change animations");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Input");
    widgets::card(ui, &pal, |ui| {
        let mut sticky = app.a11y.sticky_keys;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Keyboard,
            "Sticky keys",
            "Press shortcuts one key at a time",
            Right::Switch(&mut sticky),
        );
        if outcome.toggled() {
            if system::set_sticky_keys(sticky) {
                app.a11y.sticky_keys = sticky;
                app.notify("Sticky keys updated");
            } else {
                app.notify_error("Could not change sticky keys");
            }
        }
        widgets::divider(ui, &pal);
        let mut mouse = app.a11y.mouse_keys;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::System,
            "Mouse keys",
            "Use the numeric keypad to move the pointer",
            Right::Switch(&mut mouse),
        );
        if outcome.toggled() {
            if system::set_mouse_keys(mouse) {
                app.a11y.mouse_keys = mouse;
                app.notify("Mouse keys updated");
            } else {
                app.notify_error("Could not change mouse keys");
            }
        }
    });
}
