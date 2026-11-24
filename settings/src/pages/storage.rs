//! System > Storage: disks, mounted drives, cleaner entry.

use eframe::egui::{self, Align2, Pos2, Rect, Sense, Ui, Vec2};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self, Mount};
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn storage(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Storage") {
        app.back();
        return;
    }

    let used = app.info.disk_total_gb - app.info.disk_free_gb;
    widgets::card(ui, &pal, |ui| {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Storage,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    egui::RichText::new(format!("{:.0} GB free", app.info.disk_free_gb))
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{:.0} GB used of {:.0} GB ({:.0}%)",
                        used, app.info.disk_total_gb, app.info.disk_used_pct
                    ))
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let width = ui.available_width() - ROW_PAD;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 10.0), Sense::hover());
            ui.painter().rect_filled(rect, pal.corner(5), pal.track);
            let fraction = (app.info.disk_used_pct / 100.0).clamp(0.0, 1.0);
            let filled = Rect::from_min_size(
                rect.min,
                Vec2::new((rect.width() * fraction).max(6.0), rect.height()),
            );
            ui.painter().rect_filled(filled, pal.corner(5), pal.accent);
        });
        ui.add_space(18.0);
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Home folder");
    widgets::card(ui, &pal, |ui| {
        widgets::info_row(
            ui,
            &pal,
            "Home storage",
            &match app.info.home_used_gb {
                Some(used) => format!("{used:.1} GB used in {}", app.info.home),
                None => format!("{} · calculating usage unavailable", app.info.home),
            },
            Right::None,
        );
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Storage,
            "Open home folder",
            &app.info.home,
            Right::AccentText("Open"),
        )
        .clicked()
        {
            if !system::open_path(&app.info.home) {
                app.notify_error("Could not open the file manager");
            }
        }
        widgets::divider(ui, &pal);
        let mut recent = app.privacy.remember_recent;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Recently accessed files",
            "Remember files opened in supported applications",
            Right::Switch(&mut recent),
        );
        if outcome.toggled() {
            if system::set_remember_recent(recent) {
                app.privacy.remember_recent = recent;
                app.notify(if recent {
                    "Recently accessed files enabled"
                } else {
                    "Recently accessed files disabled"
                });
            } else {
                app.notify_error("Could not update recently accessed files");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Volumes");
    let mounts = app.info.mounts.clone();
    widgets::card(ui, &pal, |ui| {
        if mounts.is_empty() {
            widgets::empty_note(ui, &pal, "No mounted volumes were found.");
        }
        for (index, mount) in mounts.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            mount_row(ui, &pal, mount);
        }
    });
    note(
        ui,
        &pal,
        "Usage is read with df. Use Free Up Space to clear package caches and old kernels.",
    );
}

pub(crate) fn mount_row(ui: &mut Ui, pal: &Palette, mount: &Mount) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 62.0), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + ROW_PAD, rect.top() + 20.0),
        Align2::LEFT_CENTER,
        &mount.path,
        regular(13.5),
        pal.text,
    );
    ui.painter().text(
        Pos2::new(rect.right() - ROW_PAD, rect.top() + 20.0),
        Align2::RIGHT_CENTER,
        format!("{:.0} of {:.0} GB used", mount.used_gb, mount.total_gb),
        regular(12.0),
        pal.text_faint,
    );
    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + ROW_PAD, rect.top() + 40.0),
        Pos2::new(rect.right() - ROW_PAD, rect.top() + 44.0),
    );
    ui.painter().rect_filled(bar, pal.corner(2), pal.track);
    let fraction = if mount.total_gb > 0.0 {
        (mount.used_gb / mount.total_gb).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = Rect::from_min_size(
        bar.min,
        Vec2::new((bar.width() * fraction).max(4.0), bar.height()),
    );
    ui.painter().rect_filled(filled, pal.corner(2), pal.accent);
}
