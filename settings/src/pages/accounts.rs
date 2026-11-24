//! Accounts: name, photo, password, automatic login.

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::theme::{regular, semibold};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn accounts(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Accounts");

    widgets::card(ui, &pal, |ui| {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            widgets::avatar(ui, &pal, &app.info.full_name, 52.0);
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    egui::RichText::new(&app.info.full_name)
                        .font(semibold(18.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(if app.info.is_admin {
                        "Administrator"
                    } else {
                        "Standard user"
                    })
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(ROW_PAD);
                ui.spacing_mut().item_spacing.x = 8.0;
                if app.has_photo()
                    && widgets::ghost_button(ui, &pal, "Remove picture", None).clicked()
                {
                    app.load_photo(Some(""));
                }
                if widgets::button(ui, &pal, pal.accent, "Choose a picture", Some(Icon::Image))
                    .clicked()
                {
                    app.load_photo(Some("pick"));
                }
            });
        });
        ui.add_space(18.0);
    });

    spacer(ui, 16.0);
    widgets::card(ui, &pal, |ui| {
        widgets::info_row(ui, &pal, "Username", &app.info.user, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(ui, &pal, "Home folder", &app.info.home, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(ui, &pal, "Shell", &app.info.shell, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(ui, &pal, "Groups", &app.info.groups.join(", "), Right::None);
    });
    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Sign-in");
    widgets::card(ui, &pal, |ui| {
        let mut rename = false;
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Accounts,
            "Account name",
            &app.info.full_name,
            Right::AccentText("Change…"),
        )
        .clicked()
        {
            rename = true;
        }
        widgets::divider(ui, &pal);
        let mut password = false;
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Shield,
            "Password",
            "Change the password used to sign in",
            Right::AccentText("Change…"),
        )
        .clicked()
        {
            password = true;
        }
        widgets::divider(ui, &pal);
        match app.info.autologin {
            Some(mut on) => {
                let outcome = widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Power,
                    "Automatic sign-in",
                    "Sign in without asking for a password at startup",
                    Right::Switch(&mut on),
                );
                if outcome.toggled {
                    app.set_autologin(on);
                }
            }
            None => {
                widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Power,
                    "Automatic sign-in",
                    "Unavailable: GDM is not installed",
                    Right::None,
                );
            }
        }
        if rename {
            app.open_rename_account();
        }
        if password {
            app.open_change_password();
        }
    });
}
