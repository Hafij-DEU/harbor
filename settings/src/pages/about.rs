//! System > About: device name, hardware and OS details.

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::theme::{regular, semibold};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn about(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "About") {
        app.back();
        return;
    }

    widgets::card(ui, &pal, |ui| {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            widgets::avatar(ui, &pal, &app.info.full_name, 48.0);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    egui::RichText::new(&app.info.hostname)
                        .font(semibold(18.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(&app.info.os_pretty)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
            });
        });
        ui.add_space(16.0);
    });

    spacer(ui, 16.0);
    widgets::card(ui, &pal, |ui| {
        let specs = [
            ("Device name".to_owned(), app.info.hostname.clone()),
            ("Operating system".to_owned(), app.info.os_pretty.clone()),
            ("Kernel".to_owned(), app.info.kernel.clone()),
            ("Desktop".to_owned(), app.info.desktop.clone()),
            ("Processor".to_owned(), app.info.cpu.clone()),
            (
                "Memory".to_owned(),
                format!(
                    "{:.1} GB installed · {:.1} GB available",
                    app.info.mem_total_gb, app.info.mem_available_gb
                ),
            ),
            (
                "Storage".to_owned(),
                format!(
                    "{:.0} GB total · {:.0} GB free",
                    app.info.disk_total_gb, app.info.disk_free_gb
                ),
            ),
            ("Uptime".to_owned(), app.info.uptime.clone()),
        ];
        for (index, (label, value)) in specs.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            widgets::info_row(ui, &pal, label, value, Right::None);
        }
    });

    spacer(ui, 16.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        if widgets::button(ui, &pal, pal.accent, "Rename device", None).clicked() {
            app.open_rename();
        }
        if widgets::ghost_button(ui, &pal, "Copy specs", Some(Icon::Copy)).clicked() {
            ui.ctx().copy_text(app.info.summary());
            app.notify("Specifications copied to the clipboard");
        }
    });
}
