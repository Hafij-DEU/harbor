//! Privacy & security: screen lock, location, camera, microphone, history.

use eframe::egui::Ui;

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::widgets::{self, Right};

use super::*;

pub fn privacy(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Privacy & security");

    widgets::section_label(ui, &pal, "Protection");
    let mut open: Option<u8> = None;
    widgets::card(ui, &pal, |ui| {
        let subs = [
            "Firewall, updates, Secure Boot, AppArmor, encryption at a glance",
            "BIOS and device firmware from fwupd",
            "Whole PC or per app",
            "Whole PC, per app, OpenSnitch for deb apps",
            "Local ClamAV scan, quarantine",
        ];
        for (i, (title, icon, _)) in crate::app::SECURITY_PAGES.iter().enumerate() {
            if i > 0 {
                widgets::divider(ui, &pal);
            }
            if widgets::row(ui, &pal, pal.accent, *icon, title, subs[i], Right::Chevron).clicked() {
                open = Some(i as u8);
            }
        }
    });
    if let Some(i) = open {
        app.go(crate::app::View::Detail(
            crate::app::Page::Privacy,
            crate::app::Detail::Security(i),
        ));
        return;
    }
    spacer(ui, 16.0);

    widgets::section_label(ui, &pal, "Security");
    widgets::card(ui, &pal, |ui| {
        let mut on = app.privacy.screen_lock;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Shield,
            "Screen lock",
            "Ask for the password after the screen turns off",
            Right::Switch(&mut on),
        );
        if outcome.toggled() {
            if system::set_screen_lock(on) {
                app.privacy.screen_lock = on;
                app.notify(if on {
                    "Screen lock enabled"
                } else {
                    "Screen lock disabled"
                });
            } else {
                app.notify_error("Could not change the screen lock");
            }
        }
        widgets::divider(ui, &pal);
        let mut lock_notifications = app.info.lockscreen_notifications;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bell,
            "Notifications on the lock screen",
            "Show alert content before signing in",
            Right::Switch(&mut lock_notifications),
        );
        if outcome.toggled() {
            if system::set_lock_notifications(lock_notifications) {
                app.info.lockscreen_notifications = lock_notifications;
                app.notify("Lock screen notifications updated");
            } else {
                app.notify_error("Could not change lock screen notifications");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "App permissions");
    widgets::card(ui, &pal, |ui| {
        let mut location = app.privacy.location;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Globe,
            "Location services",
            "Let applications use your approximate location",
            Right::Switch(&mut location),
        );
        if outcome.toggled() {
            if system::set_location(location) {
                app.privacy.location = location;
                app.notify(if location {
                    "Location services on"
                } else {
                    "Location services off"
                });
            } else {
                app.notify_error("Could not change location services");
            }
        }
        widgets::divider(ui, &pal);
        let mut camera = app.privacy.camera;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Apps,
            "Camera",
            "Allow apps to use the camera",
            Right::Switch(&mut camera),
        );
        if outcome.toggled() {
            if system::set_camera(camera) {
                app.privacy.camera = camera;
                app.notify(if camera {
                    "Camera allowed"
                } else {
                    "Camera blocked"
                });
            } else {
                app.notify_error("Could not change the camera permission");
            }
        }
        widgets::divider(ui, &pal);
        let mut microphone = app.privacy.microphone;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sound,
            "Microphone",
            "Allow apps to use the microphone",
            Right::Switch(&mut microphone),
        );
        if outcome.toggled() {
            if system::set_microphone(microphone) {
                app.privacy.microphone = microphone;
                app.notify(if microphone {
                    "Microphone allowed"
                } else {
                    "Microphone blocked"
                });
            } else {
                app.notify_error("Could not change the microphone permission");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Usage & diagnostics");
    widgets::card(ui, &pal, |ui| {
        let mut recent = app.privacy.remember_recent;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Recently accessed files",
            "Remember recently used files",
            Right::Switch(&mut recent),
        );
        if outcome.toggled() {
            if system::set_remember_recent(recent) {
                app.privacy.remember_recent = recent;
                app.notify("File history updated");
            } else {
                app.notify_error("Could not change file history");
            }
        }
        widgets::divider(ui, &pal);
        let mut temp = app.privacy.remove_temp;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Storage,
            "Remove temporary files",
            "Delete old files from the temporary folder automatically",
            Right::Switch(&mut temp),
        );
        if outcome.toggled() {
            if system::set_remove_temp(temp) {
                app.privacy.remove_temp = temp;
                app.notify("Temporary file cleanup updated");
            } else {
                app.notify_error("Could not change temporary file cleanup");
            }
        }
        widgets::divider(ui, &pal);
        let mut report = app.privacy.report_problems;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Info,
            "Problem reporting",
            "Send anonymous crash reports to Ubuntu",
            Right::Switch(&mut report),
        );
        if outcome.toggled() {
            if system::set_report_problems(report) {
                app.privacy.report_problems = report;
                app.notify("Problem reporting updated");
            } else {
                app.notify_error("Could not change problem reporting");
            }
        }
    });
}
