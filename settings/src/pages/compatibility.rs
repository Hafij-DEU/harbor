//! Compatibility: Windows-app and format support installers.

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::theme::regular;
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn compatibility(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Compatibility");

    // (package, display name) per missing component.
    let mut install: Option<(&'static str, &'static str)> = None;
    widgets::card(ui, &pal, |ui| {
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Compatibility,
            "Wine",
            app.info
                .wine
                .as_deref()
                .map(|v| format!("Installed · {v}"))
                .unwrap_or_else(|| "Not installed".to_owned())
                .as_str(),
            if app.info.wine.is_none() {
                Right::AccentText("Install")
            } else {
                Right::None
            },
        )
        .clicked()
            && app.info.wine.is_none()
        {
            install = Some(("wine", "Wine"));
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Apps,
            "Android apps (Waydroid)",
            if app.info.waydroid {
                "Installed"
            } else {
                "Not installed"
            },
            if app.info.waydroid {
                Right::None
            } else {
                Right::AccentText("Install")
            },
        )
        .clicked()
            && !app.info.waydroid
        {
            install = Some(("waydroid", "Waydroid"));
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sparkle,
            "Steam / Proton",
            if app.info.steam {
                "Installed"
            } else {
                "Not detected"
            },
            if app.info.steam {
                Right::None
            } else {
                Right::AccentText("Install")
            },
        )
        .clicked()
            && !app.info.steam
        {
            install = Some(("steam-installer", "Steam"));
        }
    });
    if let Some((package, name)) = install {
        app.confirm = Some(crate::app::ConfirmDialog {
            title: format!("Install {name}?"),
            body: format!(
                "Ubuntu will install the {package} package from the archive through apt. \
You will be asked for your administrator password, and it can take a few minutes."
            ),
            confirm_label: "Install".to_owned(),
            kind: crate::app::ConfirmKind::InstallCompat {
                package: package.to_owned(),
                name: name.to_owned(),
            },
            busy: false,
        });
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "About compatibility");
    widgets::card(ui, &pal, |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - ROW_PAD);
                ui.label(
                    egui::RichText::new(
                        "Windows programs run through Wine or Proton, the compatibility layer bundled with Steam. \
Android applications run in a container managed by Waydroid.",
                    )
                    .font(regular(12.5))
                    .color(pal.text_muted),
                );
            });
        });
        ui.add_space(14.0);
    });
    spacer(ui, 12.0);
    if widgets::ghost_button(ui, &pal, "Re-check", Some(Icon::Update)).clicked() {
        reload(app, ui);
    }
}
