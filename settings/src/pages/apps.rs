//! Apps page: installed apps, App Hub library, favourites and updates tabs.

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use crate::app::{App, AppsTab, Page, View};
use crate::hub::HubApp;
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self, ROW_PAD};

use super::*;

pub fn apps(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Apps");
    ui.label(
        egui::RichText::new(
            "Install, update and remove applications, or launch what is already here.",
        )
        .font(regular(12.5))
        .color(pal.text_faint),
    );
    spacer(ui, 14.0);
    app_hub_controls(app, ui);
}

/// Suffix for catalogue rows: App Hub's own error text or a stale marker.
pub(crate) fn hub_note(item: &HubApp) -> String {
    if let Some(error) = &item.error {
        format!(" · {error}")
    } else if item.stale {
        " · cached version".to_owned()
    } else {
        String::new()
    }
}

/// Outcome of one of the small text buttons on a hub row.
pub(crate) fn hub_row(
    ui: &mut Ui,
    pal: &Palette,
    icon: Icon,
    texture: Option<&egui::TextureHandle>,
    title: &str,
    subtitle: &str,
    actions: &[(&str, bool)],
) -> Option<usize> {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 62.0), Sense::hover());
    if response.hovered() {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0),
        );
        ui.painter().rect_filled(bg, pal.corner(4), pal.hover);
    }

    let chip = Rect::from_center_size(
        Pos2::new(rect.left() + ROW_PAD + 14.0, rect.center().y),
        Vec2::splat(28.0),
    );
    match texture {
        Some(texture) => {
            egui::Image::new((texture.id(), chip.size()))
                .corner_radius(6.0)
                .paint_at(ui, chip);
        }
        None => widgets::icon_tile(ui.painter(), chip, icon, pal.accent, pal.on_accent, pal),
    }

    // Lay the actions out right-to-left first so the text can be clipped.
    let mut right = rect.right() - ROW_PAD;
    let mut buttons = Vec::new();
    for (index, (label, accent)) in actions.iter().enumerate().rev() {
        let galley = ui.painter().layout_no_wrap(
            (*label).to_owned(),
            regular(12.5),
            if *accent { pal.accent } else { pal.text_muted },
        );
        let w = galley.size().x + 20.0;
        let btn = Rect::from_min_size(
            Pos2::new(right - w, rect.center().y - 13.0),
            Vec2::new(w, 26.0),
        );
        buttons.push((index, btn, galley, *accent));
        right = btn.left() - 6.0;
    }

    let text_left = chip.right() + 14.0;
    let text_right = (right - 6.0).max(text_left);
    let clip = Rect::from_min_max(
        Pos2::new(text_left, rect.top()),
        Pos2::new(text_right, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(clip);
    painter.text(
        Pos2::new(text_left, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        title,
        regular(14.0),
        pal.text,
    );
    painter.text(
        Pos2::new(text_left, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        subtitle,
        regular(12.0),
        pal.text_faint,
    );

    let mut clicked = None;
    for (index, btn, galley, accent) in buttons {
        let resp = ui
            .interact(btn, response.id.with(("hub-act", index)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if resp.hovered() {
            ui.painter().rect_filled(btn, pal.corner(4), pal.hover);
        }
        let color = if accent { pal.accent } else { pal.text_muted };
        ui.painter().galley(
            Pos2::new(
                btn.center().x - galley.size().x / 2.0,
                btn.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        if resp.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SysAppAction {
    Open,
    Uninstall,
}

/// A row in the “All installed applications” inventory — like App Hub’s
/// All Apps page: real icon, version · source · size, channel chip, and
/// Open/Uninstall actions. Protected entries show no destructive action.
pub(crate) fn sys_app_row(
    ui: &mut Ui,
    pal: &Palette,
    texture: Option<&egui::TextureHandle>,
    item: &crate::inventory::SysApp,
    subtitle: &str,
) -> Option<SysAppAction> {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::hover());
    if response.hovered() {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0),
        );
        ui.painter().rect_filled(bg, pal.corner(4), pal.hover);
    }

    let chip = Rect::from_center_size(
        Pos2::new(rect.left() + ROW_PAD + 14.0, rect.center().y),
        Vec2::splat(28.0),
    );
    match texture {
        Some(texture) => {
            egui::Image::new((texture.id(), chip.size()))
                .corner_radius(6.0)
                .paint_at(ui, chip);
        }
        None => widgets::icon_tile(
            ui.painter(),
            chip,
            Icon::Apps,
            pal.accent,
            pal.on_accent,
            pal,
        ),
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Btn {
        Open,
        Uninstall,
    }
    let mut buttons: Vec<(Btn, &'static str, bool)> = Vec::new(); // (btn, label, danger)
    if !item.desktop.is_empty() {
        buttons.push((Btn::Open, "Open", false));
    }
    if item.uninstallable() {
        buttons.push((Btn::Uninstall, "Uninstall", true));
    }
    let mut right = rect.right() - ROW_PAD;
    let mut laid: Vec<(Btn, Rect, std::sync::Arc<egui::Galley>, bool)> = Vec::new();
    for (btn, label, danger) in buttons.iter().rev() {
        let galley = ui
            .painter()
            .layout_no_wrap((*label).to_owned(), regular(12.5), pal.text);
        let w = galley.size().x + 20.0;
        let btn_rect = Rect::from_min_size(
            Pos2::new(right - w, rect.center().y - 13.0),
            Vec2::new(w, 26.0),
        );
        laid.push((*btn, btn_rect, galley, *danger));
        right = btn_rect.left() - 6.0;
    }

    // Channel chip, just left of the action buttons.
    let channel_galley = ui.painter().layout_no_wrap(
        item.channel_label().to_owned(),
        regular(10.0),
        pal.text_faint,
    );
    let channel_w = channel_galley.size().x + 16.0;
    let channel_rect = Rect::from_min_size(
        Pos2::new(right - channel_w, rect.center().y - 11.0),
        Vec2::new(channel_w, 22.0),
    );
    ui.painter().rect_stroke(
        channel_rect,
        4,
        Stroke::new(1.0, pal.card_border),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(
            channel_rect.center().x - channel_galley.size().x / 2.0,
            channel_rect.center().y - channel_galley.size().y / 2.0,
        ),
        channel_galley,
        pal.text_faint,
    );

    let text_left = chip.right() + 14.0;
    let text_right = (channel_rect.left() - 8.0).max(text_left);
    let clip = Rect::from_min_max(
        Pos2::new(text_left, rect.top()),
        Pos2::new(text_right, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(clip);
    painter.text(
        Pos2::new(text_left, rect.center().y - 8.5),
        Align2::LEFT_CENTER,
        &item.name,
        semibold(13.0),
        pal.text,
    );
    painter.text(
        Pos2::new(text_left, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        subtitle,
        regular(11.5),
        pal.text_faint,
    );

    let mut clicked = None;
    for (btn, btn_rect, galley, danger) in laid {
        let resp = ui
            .interact(
                btn_rect,
                response.id.with(("sys-act", btn as usize)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        if resp.hovered() {
            ui.painter().rect_filled(btn_rect, pal.corner(4), pal.hover);
        }
        let color = if danger {
            Color32::from_rgb(0xE8, 0x8A, 0x8A)
        } else {
            pal.accent
        };
        ui.painter().galley(
            Pos2::new(
                btn_rect.center().x - galley.size().x / 2.0,
                btn_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        if resp.clicked() {
            clicked = Some(btn);
        }
    }
    clicked.map(|b| match b {
        Btn::Open => SysAppAction::Open,
        Btn::Uninstall => SysAppAction::Uninstall,
    })
}

// Windows-11 style tab strip for the Apps page: My Apps / Updates / Library /
// All Apps, badged like App Hub's own bar. The catalogue tabs hide without
// App Hub; the inventory and Ubuntu packages work standalone.

pub(crate) fn apps_tab_bar(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let hub_ok = crate::hub::available();
    if !hub_ok && matches!(app.hub.tab, AppsTab::MyApps | AppsTab::Library) {
        app.hub.tab = AppsTab::AllApps;
    }
    let current = app.hub.tab;
    let hub_updates = app.hub.apps.iter().filter(|a| a.update_available()).count();
    let apt_updates = app.info.updates.as_ref().map(|l| l.len()).unwrap_or(0);
    let badge = (hub_updates + apt_updates).max(0);

    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        ui.spacing_mut().item_spacing.x = 2.0;
        let tabs = [
            (AppsTab::MyApps, "My Apps"),
            (AppsTab::Updates, "Updates"),
            (AppsTab::Library, "Library"),
            (AppsTab::AllApps, "All Apps"),
        ];
        for (tab, label) in tabs {
            if !hub_ok && matches!(tab, AppsTab::MyApps | AppsTab::Library) {
                continue;
            }
            let count = matches!(tab, AppsTab::Updates)
                .then_some(badge)
                .filter(|n| *n > 0);
            if apps_tab_button(ui, &pal, label, count, current == tab).clicked() {
                app.hub.tab = tab;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(ROW_PAD);
            if widgets::ghost_button(ui, &pal, "Activity Log", Some(Icon::Log)).clicked() {
                app.activity_log_open = true;
            }
        });
    });
    spacer(ui, 10.0);
    widgets::divider(ui, &pal);
}

pub(crate) fn apps_tab_button(
    ui: &mut Ui,
    pal: &Palette,
    label: &str,
    badge: Option<usize>,
    active: bool,
) -> egui::Response {
    let label_font = semibold(13.5);
    let badge_font = regular(11.0);
    let text_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), label_font.clone(), pal.text);
    let text_w = text_galley.size().x;
    let badge_w = badge
        .map(|n| {
            ui.painter()
                .layout_no_wrap(n.to_string(), badge_font.clone(), pal.on_accent)
                .size()
                .x
                + 14.0
        })
        .unwrap_or(0.0);
    let gap = if badge.is_some() { 6.0 } else { 0.0 };
    let inner_w = text_w + gap + badge_w;
    let width = inner_w + 28.0;
    let height = 36.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    if !active && response.hovered() {
        ui.painter().rect_filled(rect, pal.corner(6), pal.hover);
    }

    let text_color = if active { pal.text } else { pal.text_faint };
    let mut x = rect.center().x - inner_w / 2.0;
    ui.painter().text(
        Pos2::new(x, rect.center().y - 1.0),
        Align2::LEFT_CENTER,
        label,
        label_font,
        text_color,
    );
    x += text_w + gap;
    if let Some(n) = badge {
        let badge_rect = Rect::from_center_size(
            Pos2::new(x + badge_w / 2.0, rect.center().y - 1.0),
            Vec2::new(badge_w, 18.0),
        );
        ui.painter().rect_filled(badge_rect, 9, pal.accent);
        ui.painter().text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            n.to_string(),
            badge_font,
            pal.on_accent,
        );
    }
    if active {
        let line_w = inner_w.min(rect.width() - 12.0);
        let line = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.bottom() - 1.5),
            Vec2::new(line_w, 2.5),
        );
        ui.painter().rect_filled(line, 1.25, pal.accent);
    }

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub(crate) fn render_my_apps_tab(
    app: &mut App,
    ui: &mut Ui,
    pending: &mut Option<(crate::hub::Action, String)>,
    confirm_remove: &mut Option<(String, String)>,
    update_all: &mut bool,
) {
    let pal = app.pal();

    widgets::card(ui, &pal, |ui| {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Hub,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(
                    egui::RichText::new("Application management")
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                let status = if app.hub.loading {
                    "Reading the catalogue…".to_owned()
                } else if let Some(err) = &app.hub.error {
                    err.clone()
                } else if app.hub.version.is_empty() {
                    "Ready".to_owned()
                } else {
                    let mine = app.hub.my_ids.clone();
                    let mine_apps: Vec<_> = app
                        .hub
                        .apps
                        .iter()
                        .filter(|a| mine.contains(&a.id))
                        .collect();
                    format!(
                        "Version {} · {} applications · {} installed",
                        app.hub.version,
                        mine_apps.len(),
                        mine_apps.iter().filter(|a| a.installed.is_some()).count()
                    )
                };
                ui.label(egui::RichText::new(status).font(regular(12.0)).color(
                    if app.hub.error.is_some() {
                        Color32::from_rgb(0xE8, 0x8A, 0x8A)
                    } else {
                        pal.text_faint
                    },
                ));
            });
        });
        ui.add_space(16.0);

        let mine = app.hub.my_ids.clone();
        let updates = app
            .hub
            .apps
            .iter()
            .filter(|a| mine.contains(&a.id) && a.update_available())
            .count();
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            ui.spacing_mut().item_spacing.x = 8.0;
            if updates > 0 {
                if widgets::button(
                    ui,
                    &pal,
                    pal.accent,
                    &format!("Update all ({updates})"),
                    None,
                )
                .clicked()
                {
                    *update_all = true;
                }
            }
            if !app.hub.loading
                && widgets::ghost_button(ui, &pal, "Refresh", Some(Icon::Update)).clicked()
            {
                app.hub_refresh();
            }
        });
        ui.add_space(18.0);
    });

    if app.hub.loading && app.hub.apps.is_empty() {
        spacer(ui, 14.0);
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, "Loading the application catalogue…");
        });
        return;
    }

    if let Some(err) = &app.hub.error {
        spacer(ui, 14.0);
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, err);
        });
        spacer(ui, 10.0);
        note(
            ui,
            &pal,
            "The integrated catalogue needs App Hub: “sudo apt install app-hub”.",
        );
        return;
    }

    spacer(ui, 14.0);
    let _ = widgets::search_box(ui, &pal, &mut app.hub.filter, false, "Filter applications");

    let query = app.hub.filter.trim().to_lowercase();
    let matches = |item: &HubApp| {
        query.is_empty()
            || item.name.to_lowercase().contains(&query)
            || item.id.to_lowercase().contains(&query)
            || item.summary.to_lowercase().contains(&query)
    };

    let mine = app.hub.my_ids.clone();
    // Only apps from the user's own catalogue (plus user-added My Apps);
    // third-party Library apps from other sources are excluded.
    let apps: Vec<_> = app
        .hub
        .apps
        .iter()
        .filter(|a| mine.contains(&a.id))
        .cloned()
        .collect();
    let installed = apps
        .iter()
        .filter(|a| a.ready() && matches(a))
        .cloned()
        .collect::<Vec<_>>();
    let installable = apps
        .iter()
        .filter(|a| a.installable() && matches(a))
        .cloned()
        .collect::<Vec<_>>();

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, &format!("My Apps ({})", installed.len()));
    widgets::card(ui, &pal, |ui| {
        if installed.is_empty() {
            widgets::empty_note(
                ui,
                &pal,
                if query.is_empty() {
                    "No applications from your own catalogue yet. Add one with “app-hub add --mine <owner/repo>”."
                } else {
                    "No installed application matches that filter."
                },
            );
        }
        for (index, item) in installed.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let subtitle = if item.summary.is_empty() {
                format!("Version {}", item.installed_label())
            } else {
                format!("{} · {}", item.installed_label(), item.summary)
            };
            let subtitle = format!("{}{}", subtitle, hub_note(item));
            let texture = item
                .icon
                .as_deref()
                .and_then(|path| app.app_icon(ui.ctx(), path));
            if let Some(action) = hub_row(
                ui,
                &pal,
                Icon::Hub,
                texture.as_ref(),
                &item.name,
                &subtitle,
                &[("Open", true), ("Remove", false)],
            ) {
                match action {
                    0 => *pending = Some((crate::hub::Action::Open, item.id.clone())),
                    _ => *confirm_remove = Some((item.id.clone(), item.name.clone())),
                }
            }
        }
    });

    if !installable.is_empty() {
        spacer(ui, 16.0);
        widgets::section_label(
            ui,
            &pal,
            &format!("Available to install ({})", installable.len()),
        );
        widgets::card(ui, &pal, |ui| {
            for (index, item) in installable.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let subtitle = if item.summary.is_empty() {
                    format!("Latest version {}", item.latest)
                } else {
                    format!("{} · {}", item.latest, item.summary)
                };
                let subtitle = format!("{}{}", subtitle, hub_note(item));
                let texture = item
                    .icon
                    .as_deref()
                    .and_then(|path| app.app_icon(ui.ctx(), path));
                if let Some(action) = hub_row(
                    ui,
                    &pal,
                    Icon::Hub,
                    texture.as_ref(),
                    &item.name,
                    &subtitle,
                    &[("Install", true)],
                ) {
                    if action == 0 {
                        *pending = Some((crate::hub::Action::Install, item.id.clone()));
                    }
                }
            }
        });
    }

    if !query.is_empty() && installed.is_empty() && installable.is_empty() {
        spacer(ui, 14.0);
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, "No application matches that filter.");
        });
    }
}

pub(crate) fn render_updates_tab(
    app: &mut App,
    ui: &mut Ui,
    pending: &mut Option<(crate::hub::Action, String)>,
    confirm_remove: &mut Option<(String, String)>,
) {
    let pal = app.pal();

    let (title, subtitle) = update_status(app);
    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Ubuntu packages");
    widgets::card(ui, &pal, |ui| {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Update,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    egui::RichText::new(&title)
                        .font(semibold(16.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(&subtitle)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
            });
        });
        ui.add_space(12.0);
        if apt_job_panel(app, ui, &pal) {
            ui.add_space(4.0);
        } else {
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.spacing_mut().item_spacing.x = 8.0;
                if app.updates_loading {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Checking for updates…")
                            .font(regular(12.5))
                            .color(pal.text_faint),
                    );
                } else {
                    let has_updates = app
                        .info
                        .updates
                        .as_ref()
                        .map(|l| !l.is_empty())
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(ROW_PAD);
                        if widgets::ghost_button(ui, &pal, "Software updates", Some(Icon::Apps))
                            .clicked()
                        {
                            app.go(View::Page(Page::Updates));
                        }
                    });
                }
            });
        }
        ui.add_space(16.0);
    });

    spacer(ui, 16.0);
    if !crate::hub::available() {
        widgets::section_label(ui, &pal, "Application updates");
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "App Hub is not installed. Ubuntu package updates above still work; application updates are provided by App Hub.",
            );
        });
        return;
    }
    if let Some(err) = &app.hub.error {
        widgets::section_label(ui, &pal, "Application updates");
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, err);
        });
        return;
    }
    let updates = app
        .hub
        .apps
        .iter()
        .filter(|a| a.update_available())
        .cloned()
        .collect::<Vec<_>>();
    widgets::section_label(
        ui,
        &pal,
        &format!("Application updates ({})", updates.len()),
    );
    widgets::card(ui, &pal, |ui| {
        if updates.is_empty() {
            widgets::empty_note(ui, &pal, "All integrated applications are up to date.");
        }
        for (index, item) in updates.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let subtitle = format!(
                "{} installed · {} available{}",
                item.installed_label(),
                item.latest,
                hub_note(item)
            );
            let texture = item
                .icon
                .as_deref()
                .and_then(|path| app.app_icon(ui.ctx(), path));
            if let Some(action) = hub_row(
                ui,
                &pal,
                Icon::Update,
                texture.as_ref(),
                &item.name,
                &subtitle,
                &[("Update", true), ("Remove", false)],
            ) {
                match action {
                    0 => *pending = Some((crate::hub::Action::Update, item.id.clone())),
                    _ => *confirm_remove = Some((item.id.clone(), item.name.clone())),
                }
            }
        }
    });
}

pub(crate) fn render_library_tab(
    app: &mut App,
    ui: &mut Ui,
    pending: &mut Option<(crate::hub::Action, String)>,
) {
    let pal = app.pal();

    spacer(ui, 16.0);
    widgets::card(ui, &pal, |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            widgets::icon_tile(ui.painter(), chip, Icon::Hub, pal.accent, pal.on_accent, &pal);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    egui::RichText::new("Add an application")
                        .font(semibold(15.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(
                        "A GitHub project that publishes .deb releases — paste a link, an owner/repo, or a whole account.",
                    )
                    .font(regular(11.5))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            ui.spacing_mut().item_spacing.x = 8.0;
            let width = ui.available_width() - ROW_PAD - 72.0;
            let edit = ui.add_sized(
                Vec2::new(width.max(120.0), 34.0),
                egui::TextEdit::singleline(&mut app.hub.library_input)
                    .hint_text("github.com/owner/repo · owner/repo")
                    .font(regular(13.0))
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                app.library_add();
            }
            let can_add = !app.hub.library_input.trim().is_empty() && app.hub.job.is_none();
            if can_add && widgets::button(ui, &pal, pal.accent, "Add", None).clicked() {
                app.library_add();
            }
        });
        ui.add_space(14.0);
    });

    spacer(ui, 12.0);
    let _ = widgets::search_box(ui, &pal, &mut app.hub.filter, false, "Filter the Library");
    let query = app.hub.filter.trim().to_lowercase();

    if let Some(err) = &app.hub.library_error {
        spacer(ui, 12.0);
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, err);
        });
        return;
    }
    if app.hub.loading && app.hub.library.is_empty() {
        spacer(ui, 12.0);
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, "Reading the Library…");
        });
        return;
    }

    let hub_apps: std::collections::HashMap<&str, &HubApp> =
        app.hub.apps.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut by_owner: std::collections::BTreeMap<
        String,
        Vec<(crate::hub::LibraryApp, Option<HubApp>)>,
    > = std::collections::BTreeMap::new();
    for item in &app.hub.library {
        if item.section == "mine" {
            continue;
        }
        if !query.is_empty()
            && !item.name.to_lowercase().contains(&query)
            && !item.repo.to_lowercase().contains(&query)
            && !item.summary.to_lowercase().contains(&query)
            && !item.owner.to_lowercase().contains(&query)
        {
            continue;
        }
        let owner = if item.owner.is_empty() {
            "Other".to_owned()
        } else {
            item.owner.clone()
        };
        by_owner.entry(owner).or_default().push((
            item.clone(),
            hub_apps.get(item.id.as_str()).map(|h| (*h).clone()),
        ));
    }

    if by_owner.is_empty() {
        spacer(ui, 12.0);
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                if query.is_empty() {
                    "The Library is empty. Add a GitHub project that publishes .deb releases above."
                } else {
                    "No Library application matches that filter."
                },
            );
        });
    }

    for (owner, items) in by_owner {
        spacer(ui, 16.0);
        widgets::section_label(ui, &pal, &owner);
        widgets::card(ui, &pal, |ui| {
            for (index, (item, status)) in items.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let icon = item.icon.as_deref().and_then(|i| app.app_icon(ui.ctx(), i));
                let status_text = match status {
                    Some(h) if h.update_available() => {
                        format!("{} installed · {} available", h.installed_label(), h.latest)
                    }
                    Some(h) if h.installed.is_some() => {
                        format!("Installed {}", h.installed_label())
                    }
                    _ => "Not installed".to_owned(),
                };
                let subtitle = if item.summary.is_empty() {
                    format!("{} · {}", item.repo, status_text)
                } else {
                    format!("{} · {}", item.summary, status_text)
                };
                let primary = match status {
                    Some(h) if h.update_available() => "Update",
                    Some(h) if h.installed.is_some() => "Open",
                    _ => "Install",
                };
                match library_row(ui, &pal, icon.as_ref(), item, &subtitle, primary) {
                    Some(LibraryRowAction::Primary) => {
                        let action = if primary == "Install" {
                            crate::hub::Action::Install
                        } else if primary == "Update" {
                            crate::hub::Action::Update
                        } else {
                            crate::hub::Action::Open
                        };
                        *pending = Some((action, item.id.clone()));
                    }
                    Some(LibraryRowAction::Remove) => {
                        app.library_remove(&item.id, &item.name);
                    }
                    None => {}
                }
            }
        });
    }

    render_favorites(app, ui);
}

/// App Hub's saved install recipes (its Favorites page). They are neither in
/// the catalogue nor in the Library, so before 0.5.8 they never appeared here.
pub(crate) fn render_favorites(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let favorites = app.hub.favorites.clone();
    if favorites.is_empty() {
        return;
    }
    spacer(ui, 18.0);
    widgets::section_label(ui, &pal, &format!("Saved installs ({})", favorites.len()));
    widgets::card(ui, &pal, |ui| {
        for (index, favorite) in favorites.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let texture = (!favorite.icon.is_empty())
                .then(|| app.app_icon(ui.ctx(), &favorite.icon))
                .flatten();
            let name = if favorite.name.is_empty() {
                favorite.id.clone()
            } else {
                favorite.name.clone()
            };
            let subtitle = if favorite.summary.is_empty() {
                format!("{} · {}", favorite.channel_label(), favorite.describe())
            } else {
                format!(
                    "{} · {} · {}",
                    favorite.channel_label(),
                    favorite.summary,
                    favorite.describe()
                )
            };
            let actions: &[(&str, bool)] = if favorite.installable() {
                &[("Install", true)]
            } else {
                &[]
            };
            if hub_row(
                ui,
                &pal,
                Icon::Hub,
                texture.as_ref(),
                &name,
                &subtitle,
                actions,
            )
            .is_some()
            {
                app.open_install_favorite(&favorite.key());
            }
        }
    });
    spacer(ui, 8.0);
    note(
        ui,
        &pal,
        "Saved in App Hub's Favorites. Downloads that need App Hub's helper install from App Hub itself.",
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryRowAction {
    Primary,
    Remove,
}

pub(crate) fn library_row(
    ui: &mut Ui,
    pal: &Palette,
    texture: Option<&egui::TextureHandle>,
    item: &crate::hub::LibraryApp,
    subtitle: &str,
    primary: &str,
) -> Option<LibraryRowAction> {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::hover());
    if response.hovered() {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0),
        );
        ui.painter().rect_filled(bg, pal.corner(4), pal.hover);
    }

    let chip = Rect::from_center_size(
        Pos2::new(rect.left() + ROW_PAD + 14.0, rect.center().y),
        Vec2::splat(28.0),
    );
    match texture {
        Some(texture) => {
            egui::Image::new((texture.id(), chip.size()))
                .corner_radius(6.0)
                .paint_at(ui, chip);
        }
        None => widgets::icon_tile(
            ui.painter(),
            chip,
            Icon::Hub,
            pal.accent,
            pal.on_accent,
            pal,
        ),
    }

    let mut right = rect.right() - ROW_PAD;
    let mut laid: Vec<(LibraryRowAction, Rect, std::sync::Arc<egui::Galley>, bool)> = Vec::new();
    for (action, label, strong) in [
        (LibraryRowAction::Primary, primary, true),
        (LibraryRowAction::Remove, "Remove", false),
    ]
    .into_iter()
    .rev()
    {
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_owned(), regular(12.5), pal.text);
        let w = galley.size().x + 20.0;
        let btn_rect = Rect::from_min_size(
            Pos2::new(right - w, rect.center().y - 13.0),
            Vec2::new(w, 26.0),
        );
        laid.push((action, btn_rect, galley, strong));
        right = btn_rect.left() - 6.0;
    }

    let text_left = chip.right() + 14.0;
    let text_right = (right - 6.0).max(text_left);
    let clip = Rect::from_min_max(
        Pos2::new(text_left, rect.top()),
        Pos2::new(text_right, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(clip);
    painter.text(
        Pos2::new(text_left, rect.center().y - 8.5),
        Align2::LEFT_CENTER,
        &item.name,
        semibold(13.0),
        pal.text,
    );
    painter.text(
        Pos2::new(text_left, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        subtitle,
        regular(11.5),
        pal.text_faint,
    );

    let mut clicked = None;
    for (action, btn_rect, galley, strong) in laid {
        let resp = ui
            .interact(
                btn_rect,
                response.id.with(("lib-act", action as usize)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let resp = if action == LibraryRowAction::Remove {
            resp.on_hover_text("Remove from the Library — does not uninstall the app")
        } else {
            resp
        };
        if resp.hovered() {
            ui.painter().rect_filled(btn_rect, pal.corner(4), pal.hover);
        }
        let color = if strong { pal.accent } else { pal.text_faint };
        ui.painter().galley(
            Pos2::new(
                btn_rect.center().x - galley.size().x / 2.0,
                btn_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        if resp.clicked() {
            clicked = Some(action);
        }
    }
    clicked
}

pub(crate) fn render_all_apps_tab(
    app: &mut App,
    ui: &mut Ui,
    remove_sys: &mut Option<crate::inventory::SysApp>,
    launch_desktop: &mut Option<String>,
) {
    let pal = app.pal();

    spacer(ui, 16.0);
    let _ = widgets::search_box(
        ui,
        &pal,
        &mut app.hub.filter,
        false,
        "Filter installed applications",
    );
    let sys_query = app.hub.filter.trim().to_lowercase();
    let sys_matches = |item: &crate::inventory::SysApp| {
        sys_query.is_empty()
            || item.name.to_lowercase().contains(&sys_query)
            || item.id.to_lowercase().contains(&sys_query)
            || item.source.to_lowercase().contains(&sys_query)
    };
    let sys_apps = app
        .hub
        .sys_apps
        .iter()
        .filter(|a| sys_matches(a))
        .cloned()
        .collect::<Vec<_>>();
    let sys_total = app.hub.sys_apps.len();
    widgets::section_label(
        ui,
        &pal,
        &format!("All installed applications ({sys_total})"),
    );
    if let Some(err) = &app.hub.sys_error {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, err);
        });
    } else if app.hub.sys_loading && app.hub.sys_apps.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(ui, &pal, "Reading the installed applications…");
        });
    } else if sys_apps.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                if sys_query.is_empty() {
                    "No installed application was found."
                } else {
                    "No installed application matches that filter."
                },
            );
        });
    } else {
        widgets::card(ui, &pal, |ui| {
            for (index, item) in sys_apps.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let mut parts: Vec<String> = Vec::new();
                if !item.version.is_empty() {
                    parts.push(item.version.clone());
                }
                if !item.source.is_empty() {
                    parts.push(item.source.clone());
                }
                let size = item.size_label();
                if !size.is_empty() {
                    parts.push(size);
                }
                if item.protected.is_some() {
                    parts.push("protected system component".to_owned());
                }
                let subtitle = parts.join(" · ");
                let texture = item
                    .icon
                    .as_deref()
                    .and_then(|icon| app.app_icon(ui.ctx(), icon));
                match sys_app_row(ui, &pal, texture.as_ref(), item, &subtitle) {
                    Some(SysAppAction::Open) => {
                        if !item.desktop.is_empty() {
                            *launch_desktop = Some(item.desktop.clone());
                        }
                    }
                    Some(SysAppAction::Uninstall) => {
                        *remove_sys = Some(item.clone());
                    }
                    None => {}
                }
            }
        });
        if !sys_query.is_empty() && sys_apps.len() != sys_total {
            spacer(ui, 6.0);
            note(
                ui,
                &pal,
                &format!("Showing {} of {sys_total} applications.", sys_apps.len()),
            );
        }
    }
}

pub(crate) fn app_hub_controls(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let mut pending: Option<(crate::hub::Action, String)> = None;
    let mut confirm_remove: Option<(String, String)> = None;
    let mut remove_sys: Option<crate::inventory::SysApp> = None;
    let mut launch_desktop: Option<String> = None;
    let mut update_all = false;

    apps_tab_bar(app, ui);
    spacer(ui, 12.0);

    if let Some(job) = &app.hub.job {
        widgets::card(ui, &pal, |ui| {
            crate::jobview::panel(ui, &pal, "hub-job", &job.label, &job.log, None);
        });
        spacer(ui, 4.0);
    } else if let Some(done) = &app.hub_done {
        let closed = widgets::card(ui, &pal, |ui| {
            crate::jobview::panel(ui, &pal, "hub-job", &done.label, &done.log, Some(done.ok))
        });
        if closed {
            app.hub_done = None;
        }
        spacer(ui, 4.0);
    }

    match app.hub.tab {
        AppsTab::MyApps => {
            render_my_apps_tab(app, ui, &mut pending, &mut confirm_remove, &mut update_all)
        }
        AppsTab::Updates => render_updates_tab(app, ui, &mut pending, &mut confirm_remove),
        AppsTab::Library => render_library_tab(app, ui, &mut pending),
        AppsTab::AllApps => {
            app.ensure_sys_apps();
            render_all_apps_tab(app, ui, &mut remove_sys, &mut launch_desktop)
        }
    }

    if matches!(app.hub.tab, AppsTab::MyApps | AppsTab::Library) {
        note(ui, &pal, "Installing and removing use the integrated catalogue, so the password prompt is the only interruption. Your personal files are never touched.");
    }

    if update_all {
        app.hub_update_all();
    }
    if let Some(sys) = remove_sys {
        app.open_remove_sys_app(&sys);
    } else if let Some(path) = launch_desktop {
        if system::launch_desktop(&path) {
            app.notify("Launching application");
        } else {
            app.notify_error("Could not launch that application");
        }
    }
    if let Some((id, name)) = confirm_remove {
        app.open_uninstall(&id, &name);
    } else if let Some((action, id)) = pending {
        app.hub_act(action, &id);
    }
}
