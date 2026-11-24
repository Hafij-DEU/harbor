//! Bluetooth & devices > Mouse and touchpad.

use eframe::egui::Ui;

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::Palette;
use crate::widgets::{self, Right};

use super::*;

pub fn pointer(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Mouse & touchpad") {
        app.back();
        return;
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Mouse");
    widgets::card(ui, &pal, |ui| {
        let mut speed = ((app.info.mouse_speed + 1.0) / 2.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Pointer,
            "Pointer speed",
            format!("{:.0}%", (app.info.mouse_speed + 1.0) / 2.0 * 100.0),
            &mut speed,
        );
        if changed {
            let value = speed * 2.0 - 1.0;
            if system::set_mouse_speed(value) {
                app.info.mouse_speed = value;
            } else {
                app.notify_error("Could not change the pointer speed");
            }
        }
        widgets::divider(ui, &pal);
        let mut natural = app.info.mouse_natural;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Natural scrolling",
            "Scroll the content instead of the scrollbar direction",
            Right::Switch(&mut natural),
        );
        if outcome.toggled {
            if system::set_mouse("natural-scroll", natural) {
                app.info.mouse_natural = natural;
                app.notify(if natural {
                    "Natural scrolling on"
                } else {
                    "Natural scrolling off"
                });
            } else {
                app.notify_error("Could not change scrolling");
            }
        }
        widgets::divider(ui, &pal);
        let mut left_handed = app.info.mouse_left_handed;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Left-handed",
            "Swap the primary and secondary buttons",
            Right::Switch(&mut left_handed),
        );
        if outcome.toggled {
            if system::set_mouse("left-handed", left_handed) {
                app.info.mouse_left_handed = left_handed;
                app.notify(if left_handed {
                    "Mouse is left-handed"
                } else {
                    "Mouse is right-handed"
                });
            } else {
                app.notify_error("Could not change the mouse buttons");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Touchpad");
    widgets::card(ui, &pal, |ui| {
        let mut speed = ((app.info.touchpad_speed + 1.0) / 2.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Pointer,
            "Pointer speed",
            format!("{:.0}%", (app.info.touchpad_speed + 1.0) / 2.0 * 100.0),
            &mut speed,
        );
        if changed {
            let value = speed * 2.0 - 1.0;
            if system::set_touchpad_speed(value) {
                app.info.touchpad_speed = value;
            } else {
                app.notify_error("Could not change the pointer speed");
            }
        }
        widgets::divider(ui, &pal);
        let mut tap = app.info.touchpad_tap;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Tap to click",
            "Tap the pad instead of pressing the button",
            Right::Switch(&mut tap),
        );
        if outcome.toggled {
            if system::set_touchpad("tap-to-click", tap) {
                app.info.touchpad_tap = tap;
            } else {
                app.notify_error("Could not change tap to click");
            }
        }
        widgets::divider(ui, &pal);
        let mut two_finger = app.info.touchpad_two_finger;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Two-finger scrolling",
            "Scroll with two fingers",
            Right::Switch(&mut two_finger),
        );
        if outcome.toggled {
            if system::set_touchpad("two-finger-scrolling-enabled", two_finger) {
                app.info.touchpad_two_finger = two_finger;
            } else {
                app.notify_error("Could not change two-finger scrolling");
            }
        }
        widgets::divider(ui, &pal);
        let mut natural = app.info.touchpad_natural;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Natural scrolling",
            "Content follows your fingers",
            Right::Switch(&mut natural),
        );
        if outcome.toggled {
            if system::set_touchpad("natural-scroll", natural) {
                app.info.touchpad_natural = natural;
            } else {
                app.notify_error("Could not change scrolling");
            }
        }
        widgets::divider(ui, &pal);
        let mut disable = app.info.touchpad_disable_typing;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Disable while typing",
            "Ignore the pad while you are typing",
            Right::Switch(&mut disable),
        );
        if outcome.toggled {
            if system::set_touchpad("disable-while-typing", disable) {
                app.info.touchpad_disable_typing = disable;
            } else {
                app.notify_error("Could not change disable while typing");
            }
        }
    });
    note(ui, &pal, "Applied immediately through GNOME settings.");

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Pointer");
    widgets::card(ui, &pal, |ui| pointer_size_row(app, ui, &pal));
}

pub(crate) fn pointer_size_row(app: &mut App, ui: &mut Ui, pal: &Palette) {
    let mut cursor = app.info.cursor_size as f32;
    if range_row(
        ui,
        pal,
        Icon::Pointer,
        "Pointer size",
        format!("{} px", cursor.round() as u32),
        &mut cursor,
        16.0,
        96.0,
        8.0,
    ) {
        let px = cursor.round() as u32;
        if system::set_cursor_size(px) {
            app.info.cursor_size = px;
        } else {
            app.notify_error("Could not change the pointer size");
        }
    }
}
