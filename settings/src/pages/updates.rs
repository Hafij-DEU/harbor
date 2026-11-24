//! Windows Update style page: apt, snap and flatpak updates.

use eframe::egui::{self, Sense, Ui, Vec2};

use crate::app::App;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

pub fn updates(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Software updates");

    let (title, subtitle) = update_status(app);
    widgets::card(ui, &pal, |ui| {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Update,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - ROW_PAD);
                ui.label(
                    egui::RichText::new(&title)
                        .font(semibold(18.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(&subtitle)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} · Ubuntu {} · {}",
                        app.info.hostname,
                        app.info.os_pretty.split_whitespace().nth(1).unwrap_or(""),
                        app.info.desktop
                    ))
                    .font(regular(11.5))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(16.0);
        if apt_job_panel(app, ui, &pal) {
            ui.add_space(4.0);
        } else {
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.spacing_mut().item_spacing.x = 8.0;
                if app.updates_loading {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Checking apt, snap and Flatpak for updates…")
                            .font(regular(12.5))
                            .color(pal.text_faint),
                    );
                } else {
                    let has_updates = app
                        .info
                        .updates
                        .as_ref()
                        .map(|list| !list.is_empty())
                        .unwrap_or(false);
                    if has_updates
                        && widgets::button(ui, &pal, pal.accent, "Install updates", None).clicked()
                    {
                        app.open_install_updates();
                    }
                    if widgets::ghost_button(ui, &pal, "Check for updates", Some(Icon::Update))
                        .clicked()
                    {
                        app.check_updates();
                    }
                    if widgets::ghost_button(ui, &pal, "Refresh lists", Some(Icon::Globe)).clicked()
                    {
                        app.run_refresh_lists();
                    }
                    if widgets::ghost_button(ui, &pal, "Activity Log", Some(Icon::Log)).clicked() {
                        app.activity_log_open = true;
                    }
                }
            });
        }
        let phased = app
            .info
            .updates
            .as_ref()
            .map(|list| !list.is_empty())
            .unwrap_or(false)
            && system::updates_phased();
        if phased {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.label(
                    egui::RichText::new(
                        "Ubuntu is rolling these upgrades out in stages. Install updates includes them now.",
                    )
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
        }
        ui.add_space(18.0);
    });

    if system::reboot_required() {
        spacer(ui, 14.0);
        widgets::card(ui, &pal, |ui| {
            let mut reboot = false;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Power,
                "Restart required",
                &format!("{}{}", "Some updates only take effect after a restart.", {
                    let pkgs = system::reboot_packages();
                    if pkgs.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", pkgs.join(", "))
                    }
                }),
                Right::AccentText("Restart"),
            )
            .clicked()
            {
                reboot = true;
            }
            if reboot {
                match system::schedule_reboot() {
                    Ok(message) => {
                        app.notify(&message);
                        app.record_activity("Restart requested", message, false);
                    }
                    Err(error) => {
                        app.notify_error(&error);
                        app.record_activity("Restart failed", error, true);
                    }
                }
            }
        });
    }

    if let Some(packages) = &app.info.updates {
        spacer(ui, 16.0);
        widgets::section_label(ui, &pal, "Available updates");
        let packages = packages.clone();
        widgets::card(ui, &pal, |ui| {
            if packages.is_empty() {
                widgets::empty_note(ui, &pal, "Everything is up to date.");
            }
            for (index, package) in packages.iter().take(14).enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let kind = match package.source.as_str() {
                    "snap" => "Snap app",
                    "flatpak" => "Flatpak app",
                    _ => "Ubuntu package",
                };
                let subtitle = match (package.old.is_empty(), package.new.is_empty()) {
                    (false, _) => format!(
                        "{kind} · {} installed · {} available",
                        package.old, package.new
                    ),
                    (true, false) => format!("{kind} · {} available", package.new),
                    (true, true) => format!("{kind} · update available"),
                };
                widgets::info_row(ui, &pal, &package.name, &subtitle, Right::None);
            }
        });
        if packages.len() > 14 {
            note(
                ui,
                &pal,
                &format!("…and {} more packages.", packages.len() - 14),
            );
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Update policy");
    widgets::card(ui, &pal, |ui| {
        widgets::info_row(ui, &pal, "Release", &app.info.os_pretty, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(ui, &pal, "Kernel", &app.info.kernel, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(
            ui,
            &pal,
            "Source",
            "Ubuntu archive through apt, plus GitHub releases through the integrated app catalogue",
            Right::None,
        );
        if let Some(on) = app.info.automatic_updates {
            widgets::divider(ui, &pal);
            let mut auto = on;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Shield,
                "Automatic security updates",
                "Install unattended security updates (unattended-upgrades)",
                Right::Switch(&mut auto),
            );
            if outcome.toggled {
                app.set_automatic_updates(auto);
            }
        }
    });

    spacer(ui, 14.0);
    note(
        ui,
        &pal,
        "Installing uses pkexec and apt-get upgrade with one administrator prompt. Application updates are above on the Apps page.",
    );
}

/// The running or last finished update job, drawn App Hub-style inside the
/// updates card. Returns false when there is no job to show.
pub(crate) fn apt_job_panel(app: &mut App, ui: &mut Ui, pal: &Palette) -> bool {
    if let Some(job) = &app.apt_job {
        crate::jobview::panel(ui, pal, "apt-job", &job.label, &job.log, None);
        return true;
    }
    if let Some(done) = &app.apt_done {
        if crate::jobview::panel(ui, pal, "apt-job", &done.label, &done.log, Some(done.ok)) {
            app.apt_done = None;
        }
        return true;
    }
    false
}
