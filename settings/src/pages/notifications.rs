//! System > Notifications.

use eframe::egui::Ui;

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::widgets::{self, Right};

use super::*;

pub fn notifications(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Notifications") {
        app.back();
        return;
    }
    widgets::card(ui, &pal, |ui| {
        let mut on = app.info.notifications;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bell,
            "Notifications",
            "Show banner alerts from applications",
            Right::Switch(&mut on),
        );
        if outcome.toggled {
            if system::set_notifications(on) {
                app.info.notifications = on;
                app.notify(if on {
                    "Notifications enabled"
                } else {
                    "Notifications silenced"
                });
            } else {
                app.notify_error("Could not change notification settings");
            }
        }
        widgets::divider(ui, &pal);
        let mut lock = app.info.lockscreen_notifications;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Shield,
            "Lock screen notifications",
            "Show alerts above the lock screen",
            Right::Switch(&mut lock),
        );
        if outcome.toggled {
            if system::set_lock_notifications(lock) {
                app.info.lockscreen_notifications = lock;
                app.notify("Lock screen notifications updated");
            } else {
                app.notify_error("Could not change lock screen notifications");
            }
        }
    });
    note(
        ui,
        &pal,
        "On GNOME, turning banners off is the equivalent of Do Not Disturb.",
    );
}
