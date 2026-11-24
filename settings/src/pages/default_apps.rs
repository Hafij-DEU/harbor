//! Apps > Default apps: browser and per-file-type handlers.

use eframe::egui::{self, Sense, Ui, Vec2};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::{regular, Palette};
use crate::widgets::{self, ROW_PAD};

use super::*;

pub fn default_apps(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Default applications") {
        app.back();
        return;
    }

    let mut picked: Option<(String, String, String)> = None;
    spacer(ui, 16.0);
    widgets::card(ui, &pal, |ui| {
        default_app_row(
            ui,
            &pal,
            "Web browser",
            "Open links and web pages",
            "browser",
            &app.info.default_browser,
            &app.info.browsers,
            false,
            &mut picked,
        );
        widgets::divider(ui, &pal);
        default_app_row(
            ui,
            &pal,
            "Email",
            "Open mailto: links",
            "mail",
            &app.info.default_mail,
            &app.info.mail_clients,
            false,
            &mut picked,
        );
        widgets::divider(ui, &pal);
        default_app_row(
            ui,
            &pal,
            "Text editor",
            "Open plain text files",
            "editor",
            &app.info.default_editor,
            &app.info.editors,
            false,
            &mut picked,
        );
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Files and media");
    let groups = app.info.mime_defaults.clone();
    widgets::card(ui, &pal, |ui| {
        for (index, group) in groups.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            default_app_row(
                ui,
                &pal,
                &group.title,
                &group.subtitle,
                &group.kind,
                &group.current,
                &group.apps,
                group.mixed,
                &mut picked,
            );
        }
    });

    if let Some((kind, desktop_id, _)) = picked {
        let group_mimes = app
            .info
            .mime_defaults
            .iter()
            .find(|group| group.kind == kind)
            .map(|group| group.mimes.clone());
        let ok = match kind.as_str() {
            "browser" => system::set_default_browser(&desktop_id),
            "mail" => system::set_default_for("x-scheme-handler/mailto", &desktop_id),
            "editor" => system::set_default_for("text/plain", &desktop_id),
            _ => match &group_mimes {
                Some(mimes) => system::set_default_apps(mimes, &desktop_id),
                None => false,
            },
        };
        if ok {
            match kind.as_str() {
                "browser" => app.info.default_browser = desktop_id.clone(),
                "mail" => app.info.default_mail = desktop_id.clone(),
                "editor" => app.info.default_editor = desktop_id.clone(),
                _ => {
                    if let Some(group) = app
                        .info
                        .mime_defaults
                        .iter_mut()
                        .find(|group| group.kind == kind)
                    {
                        group.current = desktop_id.clone();
                        group.mixed = false;
                    }
                }
            }
        }
        let name = std::iter::empty()
            .chain(app.info.browsers.iter())
            .chain(app.info.mail_clients.iter())
            .chain(app.info.editors.iter())
            .chain(app.info.mime_defaults.iter().flat_map(|g| g.apps.iter()))
            .find(|(id, _)| *id == desktop_id)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| desktop_id.clone());
        if ok {
            app.notify(format!("Default {kind}: {name}"));
        } else {
            app.notify_error("Could not change the default application");
        }
    }
    note(
        ui,
        &pal,
        "These are the freedesktop handlers (xdg-settings / xdg-mime), the same ones GNOME and every other desktop on Ubuntu use.",
    );
}

pub(crate) fn default_app_row(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    subtitle: &str,
    kind: &str,
    current: &str,
    apps: &[(String, String)],
    mixed: bool,
    picked: &mut Option<(String, String, String)>,
) {
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        let (chip, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        widgets::icon_tile(
            ui.painter(),
            chip,
            Icon::Apps,
            pal.accent,
            pal.on_accent,
            &pal,
        );
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                egui::RichText::new(title)
                    .font(regular(14.0))
                    .color(pal.text),
            );
            ui.label(
                egui::RichText::new(subtitle)
                    .font(regular(12.0))
                    .color(pal.text_faint),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(ROW_PAD);
            if apps.is_empty() {
                ui.label(
                    egui::RichText::new("None found")
                        .font(regular(12.5))
                        .color(pal.text_faint),
                );
                return;
            }
            let mut chosen = current.to_owned();
            let selected = if mixed {
                "Different apps".to_owned()
            } else if current.is_empty() {
                "Not set".to_owned()
            } else {
                apps.iter()
                    .find(|(id, _)| *id == current)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| current.to_owned())
            };
            egui::ComboBox::from_id_salt(kind)
                .selected_text(selected)
                .width(190.0)
                .show_ui(ui, |ui| {
                    for (id, name) in apps {
                        ui.selectable_value(&mut chosen, id.clone(), name);
                    }
                });
            if chosen != current {
                *picked = Some((kind.to_owned(), chosen, kind.to_owned()));
            }
        });
    });
    ui.add_space(10.0);
}
