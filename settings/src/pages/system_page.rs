//! System page: device summary, storage bar and the links to its sub-pages.

use eframe::egui::Ui;

use crate::app::{App, Detail, Page, View};
use crate::icons::Icon;
use crate::system::{self};
use crate::widgets::{self, Right};

use super::*;

pub fn system(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "System");
    device_header(app, ui, &pal);
    spacer(ui, 8.0);
    disk_bar(app, ui, &pal);
    spacer(ui, 8.0);
    if app.design == crate::theme::Design::Win11 {
        spacer(ui, 6.0);
        widgets::section_label(ui, &pal, "Quick settings");
        let width = ui.available_width();
        let x0 = ui.cursor().left();
        quick_settings(app, ui, &pal, width, x0, width, 1.0);
        spacer(ui, 14.0);
    }

    let outcome = widgets::row(
        ui,
        &pal,
        pal.accent,
        Icon::Update,
        "Update & drivers",
        &update_subtitle(app),
        Right::Chevron,
    );
    if outcome.clicked() {
        app.go(View::Page(Page::Updates));
    }
    spacer(ui, 14.0);

    let mut target = None;
    widgets::card(ui, &pal, |ui| {
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Display,
            "Display",
            &format!(
                "{} · {} · {} scale",
                app.info.resolution, app.info.refresh, app.info.scale
            ),
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Display);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sound,
            "Sound",
            &app.info.volume_text(),
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Sound);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bell,
            "Notifications",
            if app.info.notifications {
                "On · banner alerts enabled"
            } else {
                "Off · no banner alerts"
            },
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Notifications);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Battery,
            "Power & battery",
            &format!(
                "{} · {}",
                capitalize(&app.info.power_profile),
                app.info.battery_text()
            ),
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Power);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Storage,
            "Storage",
            &format!(
                "{:.0} GB free of {:.0} GB",
                app.info.disk_free_gb, app.info.disk_total_gb
            ),
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Storage);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Mouse & touchpad",
            "Pointer speed, scrolling and buttons",
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Pointer);
        }
        widgets::divider(ui, &pal);
        let keyboard_subtitle = if app.info.input_sources.is_empty() {
            "Key repeat and layouts".to_owned()
        } else {
            let layouts = app
                .info
                .input_sources
                .iter()
                .map(|id| {
                    system::KB_LAYOUTS
                        .iter()
                        .find(|(k, _)| *k == id)
                        .map(|(_, name)| *name)
                        .unwrap_or(id.as_str())
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{layouts} · Key repeat {}",
                if app.info.key_repeat { "on" } else { "off" }
            )
        };
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Keyboard,
            "Keyboard",
            &keyboard_subtitle,
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::Keyboard);
        }
        widgets::divider(ui, &pal);
        let default_apps_subtitle = app
            .info
            .browsers
            .iter()
            .find(|(id, _)| *id == app.info.default_browser)
            .map(|(_, name)| format!("Browser: {name}"))
            .unwrap_or_else(|| "Browser, mail and editor".to_owned());
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Apps,
            "Default applications",
            &default_apps_subtitle,
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::DefaultApps);
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Info,
            "About",
            "Device name, processor, memory, version",
            Right::Chevron,
        )
        .clicked()
        {
            target = Some(Detail::About);
        }
    });

    if let Some(detail) = target {
        app.go(View::Detail(Page::System, detail));
    }

    spacer(ui, 18.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        if widgets::ghost_button(ui, &pal, "Refresh system info", Some(Icon::Update)).clicked() {
            reload(app, ui);
        }
    });
}
