//! All pages. Data comes from `system::Info`; actions are applied immediately
//! through gsettings/pactl/nmcli/bluetoothctl and reported with a toast.

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Id, Layout, Pos2, Rect, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2,
};

use crate::app::{App, AppsTab, Detail, Page, PromptKind, View};
use crate::hub::HubApp;
use crate::icons::Icon;
use crate::system::{self, Mount};
use crate::theme::{regular, semibold, with_alpha, Palette};
use crate::widgets::{self, Right, ROW_PAD};
use std::sync::mpsc;
use std::time::Duration;

// ---------------------------------------------------------------- helpers

fn spacer(ui: &mut Ui, h: f32) {
    ui.add_space(h);
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let keep = max.saturating_sub(1);
    let head = text.chars().take(keep).collect::<String>();
    format!("{head}…")
}

fn note(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(text)
            .font(regular(12.0))
            .color(pal.text_faint),
    );
}

fn slider_row(
    ui: &mut Ui,
    pal: &Palette,
    icon: Icon,
    title: &str,
    subtitle: String,
    value: &mut f32,
) -> bool {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        let (chip, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        widgets::icon_tile(ui.painter(), chip, icon, pal.accent, pal.on_accent, &pal);
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
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
    });
    ui.add_space(2.0);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD + 42.0);
        let width = ui.available_width() - ROW_PAD - 6.0;
        ui.allocate_ui_with_layout(
            Vec2::new(width, 30.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(width);
                changed = widgets::slider(ui, pal, pal.accent, value, Id::new(title));
            },
        );
    });
    ui.add_space(12.0);
    changed
}

/// A row whose value is chosen from a drop-down list (Fluent-style row with a
/// compact combo box on the right). Returns the new value when it changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn combo_row(
    ui: &mut Ui,
    pal: &Palette,
    icon: Icon,
    title: &str,
    subtitle: &str,
    id: &str,
    options: &[(String, String)],
    current: &str,
    width: f32,
) -> Option<String> {
    let mut picked = None;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        let (chip, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        widgets::icon_tile(ui.painter(), chip, icon, pal.accent, pal.on_accent, &pal);
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
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
            let mut chosen = current.to_owned();
            let selected = options
                .iter()
                .find(|(value, _)| value == current)
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| {
                    if current.is_empty() {
                        "Not set".to_owned()
                    } else {
                        current.to_owned()
                    }
                });
            egui::ComboBox::from_id_salt(id)
                .selected_text(selected)
                .width(width)
                .show_ui(ui, |ui| {
                    for (value, label) in options {
                        ui.selectable_value(&mut chosen, value.clone(), label);
                    }
                });
            if chosen != current {
                picked = Some(chosen);
            }
        });
    });
    ui.add_space(10.0);
    picked
}

/// A slider row for an arbitrary numeric range (hours, kelvin, pixels).
#[allow(clippy::too_many_arguments)]
pub(crate) fn range_row(
    ui: &mut Ui,
    pal: &Palette,
    icon: Icon,
    title: &str,
    subtitle: String,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
) -> bool {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        let (chip, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        widgets::icon_tile(ui.painter(), chip, icon, pal.accent, pal.on_accent, &pal);
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
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
    });
    ui.add_space(2.0);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD + 42.0);
        let width = ui.available_width() - ROW_PAD - 6.0;
        ui.allocate_ui_with_layout(
            Vec2::new(width, 30.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(width);
                let mut t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
                if widgets::slider(ui, pal, pal.accent, &mut t, Id::new(title)) {
                    let raw = min + t * (max - min);
                    let stepped = (raw / step).round() * step;
                    if (stepped - *value).abs() > f32::EPSILON {
                        *value = stepped;
                        changed = true;
                    }
                }
            },
        );
    });
    ui.add_space(12.0);
    changed
}

/// Pick the closest scale the monitor actually supports.
fn nearest_scale(want: f32, scales: &[f32]) -> f32 {
    if scales.is_empty() {
        return 1.0;
    }
    scales
        .iter()
        .copied()
        .min_by(|a, b| {
            (a - want)
                .abs()
                .partial_cmp(&(b - want).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(1.0)
}

fn update_subtitle(app: &App) -> String {
    if app.updates_loading {
        "Checking for updates…".to_owned()
    } else {
        match &app.info.updates {
            Some(list) if list.is_empty() => "You're up to date".to_owned(),
            Some(list) => format!("{} updates available", list.len()),
            None => "Not checked yet".to_owned(),
        }
    }
}

fn update_status(app: &App) -> (String, String) {
    if app.updates_loading {
        return (
            "Checking for updates…".to_owned(),
            "Reading the package index with apt.".to_owned(),
        );
    }
    match &app.info.updates {
        Some(list) if list.is_empty() => (
            "You're up to date".to_owned(),
            "No package updates are waiting.".to_owned(),
        ),
        Some(list) => (
            format!("{} updates are available", list.len()),
            "Install them here with one administrator prompt.".to_owned(),
        ),
        None => (
            "Updates haven't been checked yet".to_owned(),
            "Run a check to compare installed packages with the archive.".to_owned(),
        ),
    }
}

// ---------------------------------------------------------------- system

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

pub fn reload(app: &mut App, _ui: &mut Ui) {
    // Keep the current values on screen while the refresh runs.
    app.reload_info();
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// A slim “drive” bar under the profile, like App Hub’s bottom bar:
/// disk icon, “Ubuntu: X GB free of Y GB”, a used-space meter and a
/// “Free Up Space” button that opens the App Hub-powered cleaner.
fn disk_bar(app: &mut App, ui: &mut Ui, pal: &Palette) {
    let width = ui.available_width();
    let height = 52.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(rect, pal.corner(6), pal.card);
    ui.painter().rect_stroke(
        rect,
        pal.corner(6),
        Stroke::new(1.0, pal.card_border),
        StrokeKind::Inside,
    );

    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + ROW_PAD + 12.0, rect.center().y),
        Vec2::splat(20.0),
    );
    crate::icons::paint(ui.painter(), Icon::Storage, icon_rect, pal.text_muted);

    let free = app.info.disk_free_gb;
    let total = app.info.disk_total_gb.max(0.1);
    let fraction = ((total - free) / total).clamp(0.0, 1.0);

    let text = format!("Ubuntu: {:.1} GB free of {:.1} GB", free, total);
    ui.painter().text(
        Pos2::new(icon_rect.right() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        &text,
        regular(13.0),
        pal.text,
    );

    // Right-aligned “Free Up Space” button.
    let label = "Free Up Space";
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), semibold(12.5), pal.accent);
    let btn_w = galley.size().x + 24.0 + 18.0; // text + padding + trash icon
    let btn_rect = Rect::from_min_size(
        Pos2::new(rect.right() - ROW_PAD - btn_w, rect.center().y - 15.0),
        Vec2::new(btn_w, 30.0),
    );
    let btn_resp = ui
        .interact(btn_rect, Id::new("free-up-space"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    if btn_resp.hovered() {
        ui.painter().rect_filled(btn_rect, pal.corner(5), pal.hover);
    }
    crate::icons::paint(
        ui.painter(),
        Icon::Trash,
        Rect::from_center_size(
            Pos2::new(btn_rect.left() + 14.0, btn_rect.center().y),
            Vec2::splat(13.0),
        ),
        pal.accent,
    );
    ui.painter().galley(
        Pos2::new(
            btn_rect.left() + 26.0,
            btn_rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        pal.accent,
    );

    // Used-space meter between the label and the button.
    let text_galley = ui.painter().layout_no_wrap(text, regular(13.0), pal.text);
    let bar_left = icon_rect.right() + 12.0 + text_galley.size().x + 26.0;
    let bar_right = btn_rect.left() - 18.0;
    if bar_right > bar_left + 90.0 {
        let bar = Rect::from_min_max(
            Pos2::new(bar_left, rect.center().y - 4.0),
            Pos2::new(bar_right, rect.center().y + 4.0),
        );
        ui.painter()
            .rect_filled(bar, pal.corner(3), with_alpha(pal.text, 26));
        let fill = Rect::from_min_max(
            bar.left_top(),
            Pos2::new(bar.left() + bar.width() * fraction, bar.bottom()),
        );
        let bar_color = if fraction >= 0.9 {
            Color32::from_rgb(0xE8, 0x55, 0x4F)
        } else if fraction >= 0.8 {
            Color32::from_rgb(0xE8, 0xA0, 0x33)
        } else {
            pal.accent
        };
        ui.painter().rect_filled(fill, pal.corner(3), bar_color);
        ui.painter().rect_stroke(
            bar,
            pal.corner(3),
            Stroke::new(1.0, with_alpha(pal.text, 60)),
            StrokeKind::Inside,
        );
    }

    if btn_resp.clicked() {
        app.open_cleaner();
    }
    let _ = response;
}

fn device_header(app: &mut App, ui: &mut Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 88.0), Sense::hover());
    let art = Rect::from_min_size(
        Pos2::new(rect.left() + 8.0, rect.top() + 8.0),
        Vec2::new(78.0, 62.0),
    );
    widgets::device_art(ui.painter(), art, pal, pal.accent);

    let x = art.right() + 20.0;
    ui.painter().text(
        Pos2::new(x, rect.top() + 22.0),
        Align2::LEFT_CENTER,
        &app.info.hostname,
        semibold(16.0),
        pal.text,
    );
    ui.painter().text(
        Pos2::new(x, rect.top() + 42.0),
        Align2::LEFT_CENTER,
        &app.info.device_title(),
        regular(12.0),
        pal.text_faint,
    );

    let rename_rect = Rect::from_min_size(Pos2::new(x, rect.top() + 52.0), Vec2::new(64.0, 26.0));
    let resp = ui
        .interact(rename_rect, Id::new("rename-link"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let color = if resp.hovered() {
        crate::theme::lighten(pal.accent, 24)
    } else {
        pal.accent
    };
    ui.painter().text(
        Pos2::new(x, rect.top() + 66.0),
        Align2::LEFT_CENTER,
        "Rename",
        regular(12.5),
        color,
    );
    if resp.clicked() {
        app.open_rename();
    }
}

// ---------------------------------------------------------------- details

pub fn display(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Display") {
        app.back();
        return;
    }

    // A small preview of the current mode.
    let width = ui.available_width().min(520.0);
    let height = width * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let preview = Rect::from_center_size(rect.center(), Vec2::new(width, height));
    ui.painter().rect_filled(preview, pal.corner(10), pal.card);
    ui.painter().rect_stroke(
        preview,
        pal.corner(10),
        Stroke::new(1.0, pal.card_border),
        StrokeKind::Inside,
    );
    let screen = preview.shrink(10.0);
    ui.painter()
        .rect_filled(screen, pal.corner(6), with_alpha(pal.accent, 34));
    ui.painter().text(
        screen.center() - Vec2::new(0.0, 12.0),
        Align2::CENTER_CENTER,
        &app.info.resolution,
        semibold(28.0),
        pal.text,
    );
    ui.painter().text(
        screen.center() + Vec2::new(0.0, 22.0),
        Align2::CENTER_CENTER,
        format!(
            "{} · {:.0}% scale",
            app.info.refresh,
            app.info
                .display_monitors
                .first()
                .map(|m| m.scale)
                .unwrap_or(1.0)
                * 100.0
        ),
        regular(13.0),
        pal.text_faint,
    );

    spacer(ui, 18.0);

    widgets::card(ui, &pal, |ui| {
        let mut value = app.test_brightness;
        let available = app.info.brightness.is_some();
        let subtitle = match app.info.brightness {
            Some(pct) => format!("{}%", pct),
            None => "Not available on this device".to_owned(),
        };
        let mut changed = false;
        if available {
            changed = slider_row(ui, &pal, Icon::Sun, "Brightness", subtitle, &mut value);
        } else {
            widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Sun,
                "Brightness",
                "No backlight was detected",
                Right::None,
            );
        }
        if changed {
            let pct = (value * 100.0).round() as u8;
            if Some(pct) != app.info.brightness {
                match system::set_brightness(pct) {
                    Ok(()) => {
                        app.test_brightness = value;
                        app.info.brightness = Some(pct);
                    }
                    Err(err) => app.notify_error(err),
                }
            }
        }
        widgets::divider(ui, &pal);
        let mut night = app.info.night_light;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Moon,
            "Night light",
            "Warm the screen in the evening",
            Right::Switch(&mut night),
        );
        if outcome.toggled {
            if system::set_night_light(night) {
                app.info.night_light = night;
                app.notify(if night {
                    "Night light turned on"
                } else {
                    "Night light turned off"
                });
            } else {
                app.notify_error("Could not change night light");
            }
        }
        widgets::divider(ui, &pal);
        let mut temp = app.info.night_light_temp as f32;
        let temp_label = format!("{} K", temp.round() as u32);
        if range_row(
            ui,
            &pal,
            Icon::Sun,
            "Night light strength",
            temp_label,
            &mut temp,
            1700.0,
            6500.0,
            100.0,
        ) {
            let value = temp.round() as u32;
            if system::set_night_light_temp(value) {
                app.info.night_light_temp = value;
            } else {
                app.notify_error("Could not change the night light strength");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Display mode");
    let monitors = app.info.display_monitors.clone();
    let mut apply: Option<(String, String, f32)> = None;
    if monitors.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "The desktop did not report any displays. On Wayland the compositor provides this list.",
            );
        });
    } else {
        widgets::card(ui, &pal, |ui| {
            for (index, monitor) in monitors.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                if monitors.len() > 1 {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PAD);
                        ui.label(
                            egui::RichText::new(format!("Display {}", index + 1))
                                .font(semibold(12.0))
                                .color(pal.text_muted),
                        );
                    });
                }
                let current = monitor
                    .modes
                    .iter()
                    .find(|m| m.current)
                    .or_else(|| monitor.modes.first())
                    .cloned()
                    .unwrap_or_default();

                // Resolutions
                let mut sizes: Vec<(u32, u32)> =
                    monitor.modes.iter().map(|m| (m.width, m.height)).collect();
                sizes.sort_by(|a, b| (b.0 * b.1).cmp(&(a.0 * a.1)));
                sizes.dedup();
                let res_options: Vec<(String, String)> = sizes
                    .iter()
                    .map(|(w, h)| (format!("{w}x{h}"), format!("{w} × {h}")))
                    .collect();
                let current_res = format!("{}x{}", current.width, current.height);
                if let Some(res) = combo_row(
                    ui,
                    &pal,
                    Icon::Display,
                    "Resolution",
                    "Pixels",
                    &format!("display-res-{}", monitor.connector),
                    &res_options,
                    &current_res,
                    170.0,
                ) {
                    if let Some((w, h)) = res.split_once('x') {
                        let (w, h) = (w.parse::<u32>().unwrap_or(0), h.parse::<u32>().unwrap_or(0));
                        if let Some(mode) = monitor
                            .modes
                            .iter()
                            .filter(|m| m.width == w && m.height == h)
                            .max_by(|a, b| {
                                // Prefer the rate closest to the current one.
                                (a.rate - current.rate)
                                    .abs()
                                    .partial_cmp(&(b.rate - current.rate).abs())
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                        {
                            apply = Some((
                                monitor.connector.clone(),
                                mode.id.clone(),
                                nearest_scale(monitor.scale, &mode.scales),
                            ));
                        }
                    }
                }

                // Refresh rates available at this resolution. Many panels expose
                // several modes (VRR, CEA, reduced blanking) that all round to the
                // same whole number, so keep one entry per displayed rate.
                let mut refresh_options: Vec<(String, String)> = Vec::new();
                for mode in monitor
                    .modes
                    .iter()
                    .filter(|m| m.width == current.width && m.height == current.height)
                {
                    let label = format!("{:.0} Hz", mode.rate);
                    if let Some(existing) =
                        refresh_options.iter_mut().find(|(_, text)| *text == label)
                    {
                        // Prefer the mode that is already active, else the
                        // non-VRR variant which is the plain panel mode.
                        let keep_current = mode.id == current.id;
                        let existing_is_current = existing.0 == current.id;
                        if keep_current && !existing_is_current {
                            *existing = (mode.id.clone(), label);
                        }
                    } else {
                        refresh_options.push((mode.id.clone(), label));
                    }
                }
                let refresh_hint = if refresh_options.len() <= 1 {
                    "This display reports one rate"
                } else {
                    "Updates per second"
                };
                if let Some(id) = combo_row(
                    ui,
                    &pal,
                    Icon::Clock,
                    "Refresh rate",
                    refresh_hint,
                    &format!("display-refresh-{}", monitor.connector),
                    &refresh_options,
                    &current.id,
                    150.0,
                ) {
                    if let Some(mode) = monitor.modes.iter().find(|m| m.id == id) {
                        apply = Some((
                            monitor.connector.clone(),
                            id,
                            nearest_scale(monitor.scale, &mode.scales),
                        ));
                    }
                }

                // Fractional scaling
                let scales = if current.scales.is_empty() {
                    vec![1.0]
                } else {
                    current.scales.clone()
                };
                let scale_options: Vec<(String, String)> = scales
                    .iter()
                    .map(|s| (format!("{s}"), format!("{:.0}%", s * 100.0)))
                    .collect();
                let current_scale = format!("{}", nearest_scale(monitor.scale, &scales));
                if let Some(scale) = combo_row(
                    ui,
                    &pal,
                    Icon::Personalise,
                    "Scale",
                    "Size of windows and text",
                    &format!("display-scale-{}", monitor.connector),
                    &scale_options,
                    &current_scale,
                    130.0,
                ) {
                    if let Ok(value) = scale.parse::<f32>() {
                        apply = Some((monitor.connector.clone(), current.id.clone(), value));
                    }
                }
            }
        });
    }
    if let Some((connector, mode_id, scale)) = apply {
        app.apply_display(connector, mode_id, scale);
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Night light schedule");
    widgets::card(ui, &pal, |ui| {
        let mut scheduled = app.info.night_light_scheduled;
        let schedule_hint = if scheduled {
            if app.info.night_light_auto {
                "Follow sunrise and sunset"
            } else {
                "Turn on during your own hours"
            }
        } else {
            "Off — the Night light switch applies all day"
        };
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Schedule night light",
            schedule_hint,
            Right::Switch(&mut scheduled),
        );
        if outcome.toggled {
            let from = if system::night_light_all_day(
                app.info.night_light_from,
                app.info.night_light_to,
            ) {
                system::NIGHT_LIGHT_DEFAULT_FROM
            } else {
                app.info.night_light_from
            };
            let to = if system::night_light_all_day(
                app.info.night_light_from,
                app.info.night_light_to,
            ) {
                system::NIGHT_LIGHT_DEFAULT_TO
            } else {
                app.info.night_light_to
            };
            let auto = app.info.night_light_auto && scheduled;
            if system::set_night_light_scheduled(scheduled, auto, from, to) {
                app.info.night_light_scheduled = scheduled;
                app.info.night_light_auto = auto;
                app.info.night_light_from = from;
                app.info.night_light_to = to;
                app.notify(if scheduled {
                    "Night light will follow a schedule"
                } else {
                    "Night light stays on all day"
                });
            } else {
                app.notify_error("Could not change the night light schedule");
            }
        }

        if app.info.night_light_scheduled {
            widgets::divider(ui, &pal);
            let mut auto = app.info.night_light_auto;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Moon,
                "Sunset to sunrise",
                "Follow the local sunset and sunrise times",
                Right::Switch(&mut auto),
            );
            if outcome.toggled {
                if system::set_night_light_schedule(
                    auto,
                    app.info.night_light_from,
                    app.info.night_light_to,
                ) {
                    app.info.night_light_auto = auto;
                    app.notify(if auto {
                        "Night light follows the sun"
                    } else {
                        "Night light uses your schedule"
                    });
                } else {
                    app.notify_error("Could not change the night light schedule");
                }
            }
        }

        if app.info.night_light_scheduled && !app.info.night_light_auto {
            let time_label = |value: f32| {
                format!(
                    "{:02}:{:02}",
                    value as u32,
                    ((value.fract() * 60.0).round() as u32) % 60
                )
            };
            widgets::divider(ui, &pal);
            let mut from = app.info.night_light_from;
            let from_text = time_label(from);
            if range_row(
                ui,
                &pal,
                Icon::Clock,
                "Starts at",
                from_text,
                &mut from,
                0.0,
                23.75,
                0.25,
            ) && system::set_night_light_schedule(false, from, app.info.night_light_to)
            {
                app.info.night_light_from = from;
            }
            widgets::divider(ui, &pal);
            let mut to = app.info.night_light_to;
            let to_text = time_label(to);
            if range_row(
                ui,
                &pal,
                Icon::Clock,
                "Ends at",
                to_text,
                &mut to,
                0.0,
                23.75,
                0.25,
            ) && system::set_night_light_schedule(false, app.info.night_light_from, to)
            {
                app.info.night_light_to = to;
            }
        }
    });
    if !app.info.night_light_scheduled {
        note(
            ui,
            &pal,
            "With no schedule, the Night light switch warms the screen immediately and keeps it warm until you turn it off.",
        );
    } else {
        note(
            ui,
            &pal,
            "While a schedule is set, night light only applies inside that window. Turn the schedule off to keep it on all day.",
        );
    }
    note(
        ui,
        &pal,
        "Changes are applied through the desktop's display service, so they take effect immediately.",
    );
}

pub fn sound(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Sound") {
        app.back();
        return;
    }

    if !system::audio_available() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "No audio control tool was found. Install pipewire-pulse or pulseaudio-utils.",
            );
        });
        spacer(ui, 14.0);
    }
    widgets::card(ui, &pal, |ui| {
        let mut volume = app.test_volume;
        let subtitle = if app.info.muted {
            "Muted".to_owned()
        } else {
            format!("{}%", app.info.volume)
        };
        let changed = slider_row(ui, &pal, Icon::Sound, "Volume", subtitle, &mut volume);
        if changed {
            let pct = (volume * 100.0).round() as u32;
            if pct != app.info.volume {
                if system::set_volume(pct) {
                    app.info.volume = pct;
                    app.test_volume = volume;
                    if app.info.muted {
                        system::set_muted(false);
                        app.info.muted = false;
                    }
                } else {
                    app.notify_error("Could not change the volume");
                }
            }
        }
        widgets::divider(ui, &pal);
        let mut muted = app.info.muted;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sound,
            "Mute",
            "Silence all audio output",
            Right::Switch(&mut muted),
        );
        if outcome.toggled {
            if system::set_muted(muted) {
                app.info.muted = muted;
                app.notify(if muted { "Muted" } else { "Unmuted" });
            } else {
                app.notify_error("Could not mute audio");
            }
        }
        widgets::divider(ui, &pal);
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sparkle,
            "Test output",
            "Play a short system sound",
            Right::Chevron,
        )
        .clicked()
        {
            system::test_sound();
            app.notify("Playing a test sound");
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Output");
    let sinks = app.info.sinks.clone();
    if sinks.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "No audio outputs were reported by PulseAudio/PipeWire.",
            );
        });
    } else {
        let mut selected = None;
        widgets::card(ui, &pal, |ui| {
            for (index, sink) in sinks.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let is_default = sink.name == app.info.default_sink;
                if widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Sound,
                    &sink.description,
                    &sink.name,
                    if is_default {
                        Right::Check
                    } else {
                        Right::None
                    },
                )
                .clicked()
                    && !is_default
                {
                    selected = Some(sink.name.clone());
                }
            }
        });
        if let Some(name) = selected {
            if system::set_default_sink(&name) {
                app.info.default_sink = name;
                app.notify("Default output changed");
            } else {
                app.notify_error("Could not switch output device");
            }
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Input");
    widgets::card(ui, &pal, |ui| {
        let mut mic = app.test_mic_volume;
        let subtitle = if app.info.input_muted {
            "Microphone muted".to_owned()
        } else {
            format!("{}%", app.info.microphone_volume)
        };
        let changed = slider_row(
            ui,
            &pal,
            Icon::Sound,
            "Microphone volume",
            subtitle,
            &mut mic,
        );
        if changed {
            let pct = (mic * 100.0).round() as u32;
            if pct != app.info.microphone_volume {
                if system::set_input_volume(pct) {
                    app.info.microphone_volume = pct;
                    app.test_mic_volume = mic;
                    if app.info.input_muted {
                        system::set_input_muted(false);
                        app.info.input_muted = false;
                    }
                } else {
                    app.notify_error("Could not change the microphone volume");
                }
            }
        }
        widgets::divider(ui, &pal);
        let mut muted = app.info.input_muted;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sound,
            "Mute microphone",
            "Silence all audio input",
            Right::Switch(&mut muted),
        );
        if outcome.toggled {
            if system::set_input_muted(muted) {
                app.info.input_muted = muted;
                app.notify(if muted {
                    "Microphone muted"
                } else {
                    "Microphone on"
                });
            } else {
                app.notify_error("Could not mute the microphone");
            }
        }
    });

    spacer(ui, 14.0);
    let sources = app.info.sources.clone();
    if !sources.is_empty() {
        widgets::section_label(ui, &pal, "Input device");
        let mut selected = None;
        widgets::card(ui, &pal, |ui| {
            for (index, source) in sources.iter().enumerate() {
                if index > 0 {
                    widgets::divider(ui, &pal);
                }
                let is_default = source.name == app.info.default_source;
                if widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Sound,
                    &source.description,
                    &source.name,
                    if is_default {
                        Right::Check
                    } else {
                        Right::None
                    },
                )
                .clicked()
                    && !is_default
                {
                    selected = Some(source.name.clone());
                }
            }
        });
        if let Some(name) = selected {
            if system::set_default_source(&name) {
                app.info.default_source = name;
                app.notify("Default input changed");
            } else {
                app.notify_error("Could not switch the input device");
            }
        }
    }
}

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

pub fn power(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Power & battery") {
        app.back();
        return;
    }

    widgets::card(ui, &pal, |ui| {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Battery,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let big = match app.info.battery {
                    Some(pct) => format!("{}%", pct),
                    None => "No battery".to_owned(),
                };
                ui.label(
                    egui::RichText::new(big)
                        .font(semibold(22.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(if app.info.battery.is_some() {
                        if app.info.charging {
                            "Charging"
                        } else {
                            "On battery power"
                        }
                    } else {
                        "This device runs on AC power"
                    })
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let width = ui.available_width() - ROW_PAD;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
            let fraction = app.info.battery.unwrap_or(100) as f32 / 100.0;
            ui.painter().rect_filled(rect, pal.corner(4), pal.track);
            let filled = Rect::from_min_size(
                rect.min,
                Vec2::new((rect.width() * fraction).max(6.0), rect.height()),
            );
            ui.painter().rect_filled(filled, pal.corner(4), pal.accent);
        });
        ui.add_space(16.0);
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Power mode");
    let modes = [
        (
            "power-saver",
            "Power saver",
            "Reduce performance to save battery",
        ),
        (
            "balanced",
            "Balanced",
            "Automatic balance of power and performance",
        ),
        (
            "performance",
            "Performance",
            "Favour performance over battery life",
        ),
    ];
    let mut chosen = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (key, label, subtitle)) in modes.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let selected = app.info.power_profile == *key;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Power,
                label,
                subtitle,
                if selected { Right::Check } else { Right::None },
            )
            .clicked()
                && !selected
            {
                chosen = Some(*key);
            }
        }
    });
    if let Some(key) = chosen {
        if system::set_power_profile(key) {
            app.info.power_profile = key.to_owned();
            app.notify(format!(
                "Power mode set to {}",
                capitalize(&key.replace('-', " "))
            ));
        } else {
            app.notify_error("powerprofilesctl is not available");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Screen and sleep");
    let options = [
        (300u32, "5 minutes"),
        (600, "10 minutes"),
        (900, "15 minutes"),
        (1800, "30 minutes"),
    ];
    let mut chosen = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (seconds, label)) in options.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let selected = app.info.idle_delay == *seconds;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Clock,
                label,
                "Turn the screen off after this idle period",
                if selected { Right::Check } else { Right::None },
            )
            .clicked()
                && !selected
            {
                chosen = Some(*seconds);
            }
        }
    });
    if let Some(seconds) = chosen {
        if system::set_idle_delay(seconds) {
            app.info.idle_delay = seconds;
            app.notify("Screen timeout updated");
        } else {
            app.notify_error("Could not change the screen timeout");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Automatic suspend");
    widgets::card(ui, &pal, |ui| {
        let mut on_ac = app.info.sleep_ac;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Power,
            "Suspend on AC power",
            "Sleep automatically when idle on wall power",
            Right::Switch(&mut on_ac),
        );
        if outcome.toggled {
            if system::set_sleep_idle(on_ac, app.info.sleep_battery) {
                app.info.sleep_ac = on_ac;
                app.notify(if on_ac {
                    "Automatic suspend on AC enabled"
                } else {
                    "Automatic suspend on AC disabled"
                });
            } else {
                app.notify_error("Could not change automatic suspend");
            }
        }
        widgets::divider(ui, &pal);
        let mut on_battery = app.info.sleep_battery;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Power,
            "Suspend on battery",
            "Sleep automatically when idle on battery",
            Right::Switch(&mut on_battery),
        );
        if outcome.toggled {
            if system::set_sleep_idle(app.info.sleep_ac, on_battery) {
                app.info.sleep_battery = on_battery;
                app.notify(if on_battery {
                    "Automatic suspend on battery enabled"
                } else {
                    "Automatic suspend on battery disabled"
                });
            } else {
                app.notify_error("Could not change automatic suspend");
            }
        }
    });
    note(
        ui,
        &pal,
        "Uses GNOME's automatic suspend settings; the delay is unchanged.",
    );

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "When the lid is closed");
    let lid_options = [("suspend", "Suspend"), ("nothing", "Do nothing")];
    let mut lid_change = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (title, key)) in [("On AC power", "ac"), ("On battery", "battery")]
            .into_iter()
            .enumerate()
        {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.label(
                    egui::RichText::new(title)
                        .font(semibold(12.0))
                        .color(pal.text_muted),
                );
            });
            for (value, label) in lid_options {
                let current = if key == "ac" {
                    &app.info.lid_ac
                } else {
                    &app.info.lid_battery
                };
                let selected = current == value;
                if widgets::row(
                    ui,
                    &pal,
                    pal.accent,
                    Icon::Power,
                    label,
                    if key == "ac" {
                        "When the lid is closed on wall power"
                    } else {
                        "When the lid is closed on battery"
                    },
                    if selected { Right::Check } else { Right::None },
                )
                .clicked()
                    && !selected
                {
                    lid_change = Some((key.to_owned(), value.to_owned()));
                }
            }
        }
    });
    if let Some((kind, value)) = lid_change {
        let (ac, battery) = if kind == "ac" {
            (value.clone(), app.info.lid_battery.clone())
        } else {
            (app.info.lid_ac.clone(), value.clone())
        };
        if system::set_lid_action(&ac, &battery) {
            app.info.lid_ac = ac;
            app.info.lid_battery = battery;
            app.notify("Lid action updated");
        } else {
            app.notify_error("Could not change the lid action");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Power button");
    let power_options = [
        ("suspend", "Sleep"),
        ("hibernate", "Hibernate"),
        ("interactive", "Ask what to do"),
        ("nothing", "Do nothing"),
    ];
    let power_choices: Vec<(String, String)> = power_options
        .iter()
        .map(|(value, label)| ((*value).to_owned(), (*label).to_owned()))
        .collect();
    if let Some(action) = combo_row(
        ui,
        &pal,
        Icon::Power,
        "When I press the power button",
        "Applies to the hardware power button",
        "power-button-action",
        &power_choices,
        &app.info.power_button,
        180.0,
    ) {
        if system::set_power_button(&action) {
            app.info.power_button = action;
            app.notify("Power button action updated");
        } else {
            app.notify_error("Could not change the power button action");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Battery");
    widgets::card(ui, &pal, |ui| {
        let mut saver = app.info.low_battery_saver;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Battery,
            "Power saver at low battery",
            "Switch to the power-saving profile automatically",
            Right::Switch(&mut saver),
        );
        if outcome.toggled {
            if system::set_low_battery_saver(saver) {
                app.info.low_battery_saver = saver;
                app.notify(if saver {
                    "Low-battery power saver enabled"
                } else {
                    "Low-battery power saver disabled"
                });
            } else {
                app.notify_error("Could not change the low-battery setting");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Top bar");
    widgets::card(ui, &pal, |ui| {
        let mut pct = app.info.battery_percentage;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Battery,
            "Battery percentage",
            "Show the percentage next to the battery icon",
            Right::Switch(&mut pct),
        );
        if outcome.toggled {
            if system::set_clock_detail("show-battery-percentage", pct) {
                app.info.battery_percentage = pct;
            } else {
                app.notify_error("Could not change the battery percentage");
            }
        }
    });
}

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

fn mount_row(ui: &mut Ui, pal: &Palette, mount: &Mount) {
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

// ---------------------------------------------------------------- new detail pages

pub fn pointer(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Mouse & touchpad") {
        app.back();
        return;
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Mouse");
    widgets::card(ui, &pal, |ui| {
        let mut speed = ((app.info.mouse_speed + 1.0) / 2.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Pointer,
            "Pointer speed",
            format!("{:.0}%", (app.info.mouse_speed + 1.0) / 2.0 * 100.0),
            &mut speed,
        );
        if changed {
            let value = speed * 2.0 - 1.0;
            if system::set_mouse_speed(value) {
                app.info.mouse_speed = value;
            } else {
                app.notify_error("Could not change the pointer speed");
            }
        }
        widgets::divider(ui, &pal);
        let mut natural = app.info.mouse_natural;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Natural scrolling",
            "Scroll the content instead of the scrollbar direction",
            Right::Switch(&mut natural),
        );
        if outcome.toggled {
            if system::set_mouse("natural-scroll", natural) {
                app.info.mouse_natural = natural;
                app.notify(if natural {
                    "Natural scrolling on"
                } else {
                    "Natural scrolling off"
                });
            } else {
                app.notify_error("Could not change scrolling");
            }
        }
        widgets::divider(ui, &pal);
        let mut left_handed = app.info.mouse_left_handed;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Left-handed",
            "Swap the primary and secondary buttons",
            Right::Switch(&mut left_handed),
        );
        if outcome.toggled {
            if system::set_mouse("left-handed", left_handed) {
                app.info.mouse_left_handed = left_handed;
                app.notify(if left_handed {
                    "Mouse is left-handed"
                } else {
                    "Mouse is right-handed"
                });
            } else {
                app.notify_error("Could not change the mouse buttons");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Touchpad");
    widgets::card(ui, &pal, |ui| {
        let mut speed = ((app.info.touchpad_speed + 1.0) / 2.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Pointer,
            "Pointer speed",
            format!("{:.0}%", (app.info.touchpad_speed + 1.0) / 2.0 * 100.0),
            &mut speed,
        );
        if changed {
            let value = speed * 2.0 - 1.0;
            if system::set_touchpad_speed(value) {
                app.info.touchpad_speed = value;
            } else {
                app.notify_error("Could not change the pointer speed");
            }
        }
        widgets::divider(ui, &pal);
        let mut tap = app.info.touchpad_tap;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Tap to click",
            "Tap the pad instead of pressing the button",
            Right::Switch(&mut tap),
        );
        if outcome.toggled {
            if system::set_touchpad("tap-to-click", tap) {
                app.info.touchpad_tap = tap;
            } else {
                app.notify_error("Could not change tap to click");
            }
        }
        widgets::divider(ui, &pal);
        let mut two_finger = app.info.touchpad_two_finger;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Two-finger scrolling",
            "Scroll with two fingers",
            Right::Switch(&mut two_finger),
        );
        if outcome.toggled {
            if system::set_touchpad("two-finger-scrolling-enabled", two_finger) {
                app.info.touchpad_two_finger = two_finger;
            } else {
                app.notify_error("Could not change two-finger scrolling");
            }
        }
        widgets::divider(ui, &pal);
        let mut natural = app.info.touchpad_natural;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Natural scrolling",
            "Content follows your fingers",
            Right::Switch(&mut natural),
        );
        if outcome.toggled {
            if system::set_touchpad("natural-scroll", natural) {
                app.info.touchpad_natural = natural;
            } else {
                app.notify_error("Could not change scrolling");
            }
        }
        widgets::divider(ui, &pal);
        let mut disable = app.info.touchpad_disable_typing;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Pointer,
            "Disable while typing",
            "Ignore the pad while you are typing",
            Right::Switch(&mut disable),
        );
        if outcome.toggled {
            if system::set_touchpad("disable-while-typing", disable) {
                app.info.touchpad_disable_typing = disable;
            } else {
                app.notify_error("Could not change disable while typing");
            }
        }
    });
    note(ui, &pal, "Applied immediately through GNOME settings.");

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Pointer");
    widgets::card(ui, &pal, |ui| pointer_size_row(app, ui, &pal));
}

pub fn keyboard(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    if widgets::h1_back(ui, &pal, "Keyboard") {
        app.back();
        return;
    }

    spacer(ui, 16.0);
    keymap_section(app, ui, &pal);
    spacer(ui, 20.0);
    widgets::section_label(ui, &pal, "Input sources");
    // Cycle layout with Super+Space / Super+Shift+Space, like GNOME.
    let mut remove: Option<String> = None;
    let mut add: Option<String> = None;
    let mut filter = app.keyboard_filter.clone();
    widgets::card(ui, &pal, |ui| {
        let sources = app.info.input_sources.clone();
        if sources.is_empty() {
            widgets::empty_note(ui, &pal, "No input sources reported.");
        }
        for (index, id) in sources.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let name = system::KB_LAYOUTS
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, name)| *name)
                .unwrap_or(id.as_str());
            let can_remove = sources.len() > 1;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Keyboard,
                name,
                id,
                if can_remove {
                    Right::AccentText("Remove")
                } else {
                    Right::None
                },
            );
            if can_remove && outcome.clicked() {
                remove = Some(id.clone());
            }
        }
        widgets::divider(ui, &pal);
        _ = widgets::search_box(ui, &pal, &mut filter, false, "Search layouts, e.g. German");
        let query = filter.trim().to_lowercase();
        let mut shown = 0;
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .id_salt("kb-layouts")
            .show(ui, |ui| {
                for (id, name) in system::KB_LAYOUTS {
                    if sources.iter().any(|s| s == id) {
                        continue;
                    }
                    let hay = format!("{id} {name}").to_lowercase();
                    if !query.is_empty() && !hay.contains(&query) {
                        continue;
                    }
                    shown += 1;
                    if widgets::row(
                        ui,
                        &pal,
                        pal.accent,
                        Icon::Keyboard,
                        name,
                        id,
                        Right::AccentText("Add"),
                    )
                    .clicked()
                    {
                        add = Some((*id).to_owned());
                    }
                }
                if shown == 0 {
                    widgets::empty_note(
                        ui,
                        &pal,
                        if query.is_empty() {
                            "Every listed layout is already added."
                        } else {
                            "No layout matches that filter."
                        },
                    );
                }
            });
    });
    if filter != app.keyboard_filter {
        app.keyboard_filter = filter;
    }
    if let Some(id) = remove {
        let remaining: Vec<String> = app
            .info
            .input_sources
            .iter()
            .filter(|s| **s != id)
            .cloned()
            .collect();
        if system::set_input_sources(&remaining) {
            app.info.input_sources = remaining;
            app.notify("Input source removed");
        } else {
            app.notify_error("Could not change the input sources");
        }
    }
    if let Some(id) = add {
        let mut next = app.info.input_sources.clone();
        if !next.contains(&id) {
            next.push(id);
        }
        if system::set_input_sources(&next) {
            app.info.input_sources = next;
            app.keyboard_filter.clear();
            app.notify("Input source added");
        } else {
            app.notify_error("Could not change the input sources");
        }
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Key repeat");
    widgets::card(ui, &pal, |ui| {
        let mut repeat = app.info.key_repeat;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Keyboard,
            "Key repeat",
            "Hold a key to repeat it",
            Right::Switch(&mut repeat),
        );
        if outcome.toggled {
            if system::set_key_repeat(repeat) {
                app.info.key_repeat = repeat;
            } else {
                app.notify_error("Could not change key repeat");
            }
        }
        widgets::divider(ui, &pal);
        let mut delay = ((app.info.repeat_delay as f32 - 100.0) / 900.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Keyboard,
            "Delay",
            format!("{} ms before repeat starts", app.info.repeat_delay),
            &mut delay,
        );
        if changed {
            let ms = (100.0 + delay * 900.0).round() as u32;
            if system::set_repeat_delay(ms) {
                app.info.repeat_delay = ms;
            } else {
                app.notify_error("Could not change the repeat delay");
            }
        }
        widgets::divider(ui, &pal);
        let mut interval = ((app.info.repeat_interval as f32 - 10.0) / 110.0).clamp(0.0, 1.0);
        let changed = slider_row(
            ui,
            &pal,
            Icon::Keyboard,
            "Repeat rate",
            format!("{} ms between repeats", app.info.repeat_interval),
            &mut interval,
        );
        if changed {
            let ms = (10.0 + interval * 110.0).round() as u32;
            if system::set_repeat_interval(ms) {
                app.info.repeat_interval = ms;
            } else {
                app.notify_error("Could not change the repeat rate");
            }
        }
    });
    note(ui, &pal, "Cycle input sources with Super+Space.");
}

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

fn default_app_row(
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

// ---------------------------------------------------------------- other pages

pub fn bluetooth(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Bluetooth & devices");

    widgets::card(ui, &pal, |ui| {
        let mut on = app.info.bluetooth_on;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bluetooth,
            "Bluetooth",
            if on {
                "On · discoverable by paired devices"
            } else {
                "Off"
            },
            Right::Switch(&mut on),
        );
        if outcome.toggled {
            match system::set_bluetooth(on) {
                Ok(()) => {
                    app.info.bluetooth_on = on;
                    app.notify(if on {
                        "Bluetooth turned on"
                    } else {
                        "Bluetooth turned off"
                    });
                }
                Err(err) => {
                    app.notify_error(if err.is_empty() {
                        "Could not change Bluetooth".to_owned()
                    } else {
                        err
                    });
                }
            }
        }
        if app.info.bluetooth_on {
            widgets::divider(ui, &pal);
            let mut discoverable = app.info.discoverable;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Bluetooth,
                "Discoverable",
                "Let nearby devices find this computer",
                Right::Switch(&mut discoverable),
            );
            if outcome.toggled {
                app.set_discoverable(discoverable);
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Devices");
    let devices = app.info.bluetooth_devices.clone();
    let mut device_action: Option<(String, String, BluetoothAction)> = None;
    widgets::card(ui, &pal, |ui| {
        if devices.is_empty() {
            widgets::empty_note(
                ui,
                &pal,
                "No Bluetooth devices found. Use Scan for devices below.",
            );
        }
        for (index, device) in devices.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let action = if device.connected {
                ("Disconnect", BluetoothAction::Disconnect)
            } else if device.paired {
                ("Connect", BluetoothAction::Connect)
            } else {
                ("Pair", BluetoothAction::Pair)
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                if device.connected {
                    Icon::Bluetooth
                } else {
                    Icon::Keyboard
                },
                &device.name,
                &format!(
                    "{} · {}",
                    device.mac,
                    if device.connected {
                        "Connected"
                    } else {
                        "Not connected"
                    }
                ),
                Right::AccentText(action.0),
            )
            .clicked()
            {
                device_action = Some((device.mac.clone(), device.name.clone(), action.1));
            }
        }
    });

    spacer(ui, 10.0);
    let scan_label = if app.bt_scanning {
        "Scanning…"
    } else {
        "Scan for devices"
    };
    let scan = widgets::ghost_button(ui, &pal, scan_label, Some(Icon::Bluetooth));
    if scan.clicked() && !app.bt_scanning {
        app.start_bluetooth_scan(ui.ctx());
    }
    if let Some((mac, name, action)) = device_action {
        let result = match action {
            BluetoothAction::Connect => system::connect_bluetooth(&mac, true),
            BluetoothAction::Disconnect => system::connect_bluetooth(&mac, false),
            BluetoothAction::Pair => system::pair_bluetooth(&mac),
        };
        match result {
            Ok(()) => {
                app.notify(match action {
                    BluetoothAction::Connect => format!("Connected to {name}"),
                    BluetoothAction::Disconnect => format!("Disconnected from {name}"),
                    BluetoothAction::Pair => format!("Paired with {name}"),
                });
                reload(app, ui);
            }
            Err(error) => app.notify_error(if error.is_empty() {
                format!("Could not update {name}")
            } else {
                error
            }),
        }
    }
}

#[derive(Clone, Copy)]
enum BluetoothAction {
    Connect,
    Disconnect,
    Pair,
}

pub fn network(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Network & internet");

    let devices = app.info.net_devices.clone();
    widgets::card(ui, &pal, |ui| {
        if devices.is_empty() {
            widgets::empty_note(ui, &pal, "NetworkManager reported no devices.");
        }
        for (index, device) in devices.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let title = if device.connection.is_empty() {
                device.name.clone()
            } else {
                device.connection.clone()
            };
            let mut subtitle = format!("{} · {}", device.kind, device.state);
            if !device.address.is_empty() {
                subtitle.push_str(&format!(" · {}", device.address));
            }
            widgets::row(
                ui,
                &pal,
                pal.accent,
                if device.kind.contains("wifi") {
                    Icon::Wifi
                } else {
                    Icon::Globe
                },
                &title,
                &subtitle,
                Right::None,
            );
        }
    });

    spacer(ui, 14.0);
    widgets::card(ui, &pal, |ui| {
        let mut on = app.info.wifi_enabled;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Wifi,
            "Wi-Fi",
            if on {
                "On · the radio scans for networks"
            } else {
                "Off · the radio is disabled"
            },
            Right::Switch(&mut on),
        );
        if outcome.toggled {
            app.set_wifi_radio(on);
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Proxy");
    let mut proxy_mode: Option<&'static str> = None;
    widgets::card(ui, &pal, |ui| {
        for (index, (value, label, subtitle)) in [
            ("none", "Off", "Connect to the internet directly"),
            ("manual", "Manual", "Use the proxy servers configured below"),
            (
                "auto",
                "Automatic",
                "Use the proxy configuration script from the network",
            ),
        ]
        .iter()
        .enumerate()
        {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let selected = app.info.proxy_mode == *value;
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Globe,
                label,
                subtitle,
                if selected { Right::Check } else { Right::None },
            )
            .clicked()
                && !selected
            {
                proxy_mode = Some(*value);
            }
        }
        if app.info.proxy_mode == "manual" {
            widgets::divider(ui, &pal);
            let http = if app.info.proxy_http_port > 0 {
                format!(
                    "{}:{}",
                    if app.info.proxy_http_host.is_empty() {
                        "Not set"
                    } else {
                        &app.info.proxy_http_host
                    },
                    app.info.proxy_http_port
                )
            } else {
                "Not set".to_owned()
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Globe,
                "HTTP and HTTPS proxy",
                &http,
                Right::AccentText("Edit…"),
            )
            .clicked()
            {
                app.open_proxy_prompt(PromptKind::ProxyHttp);
            }
            widgets::divider(ui, &pal);
            let socks = if app.info.proxy_socks_port > 0 {
                format!(
                    "{}:{}",
                    if app.info.proxy_socks_host.is_empty() {
                        "Not set"
                    } else {
                        &app.info.proxy_socks_host
                    },
                    app.info.proxy_socks_port
                )
            } else {
                "Not set".to_owned()
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Globe,
                "SOCKS proxy",
                &socks,
                Right::AccentText("Edit…"),
            )
            .clicked()
            {
                app.open_proxy_prompt(PromptKind::ProxySocks);
            }
            widgets::divider(ui, &pal);
            let ignore = if app.info.proxy_ignore.trim().is_empty() {
                "None".to_owned()
            } else {
                shorten(&app.info.proxy_ignore, 40)
            };
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Shield,
                "Ignored hosts",
                &ignore,
                Right::AccentText("Edit…"),
            )
            .clicked()
            {
                app.open_proxy_prompt(PromptKind::ProxyIgnore);
            }
        }
    });
    if let Some(mode) = proxy_mode {
        app.set_proxy_mode(mode);
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Wi-Fi networks");
    let networks = app.info.wifi_networks.clone();
    let mut join: Option<(String, bool)> = None;
    widgets::card(ui, &pal, |ui| {
        if networks.is_empty() {
            widgets::empty_note(
                ui,
                &pal,
                "No Wi-Fi networks were reported. Refresh to scan again.",
            );
        }
        for (index, network) in networks.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let secured = !network.security.is_empty() && network.security != "--";
            let subtitle = format!(
                "{} · {}% signal{}",
                if secured { "Secured" } else { "Open" },
                network.signal,
                if network.active { " · Connected" } else { "" }
            );
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Wifi,
                &network.ssid,
                &subtitle,
                if network.active {
                    Right::Check
                } else {
                    Right::AccentText("Connect")
                },
            )
            .clicked()
                && !network.active
            {
                join = Some((network.ssid.clone(), secured));
            }
        }
    });
    if let Some(active) = networks.iter().find(|n| n.active) {
        spacer(ui, 16.0);
        widgets::section_label(ui, &pal, "Connected network");
        widgets::card(ui, &pal, |ui| {
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Wifi,
                &active.ssid,
                "Forget the saved connection, including its password",
                Right::AccentText("Forget"),
            );
            if outcome.clicked() {
                app.open_forget_wifi();
            }
        });
    }

    spacer(ui, 10.0);
    if widgets::ghost_button(ui, &pal, "Refresh networks", Some(Icon::Update)).clicked() {
        reload(app, ui);
    }
    if let Some((ssid, secured)) = join {
        app.open_wifi_connect(&ssid, secured);
    }
    note(
        ui,
        &pal,
        "NetworkManager connects to the selected Wi-Fi network. Passwords are sent only to NetworkManager and are never saved by Settings.",
    );
}

pub fn personalisation(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Personalisation");

    // The two interfaces share every setting; this only changes the chrome,
    // the palette and the Windows 10 home screen.
    widgets::section_label(ui, &pal, "Interface");
    let mut chosen = app.design;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for design in [crate::theme::Design::Win11, crate::theme::Design::Win10] {
            let selected = app.design == design;
            let desc = if design == crate::theme::Design::Win10 {
                "Category home, sub-page pane, soft corners"
            } else {
                "Rounded cards, accent tiles, sidebar"
            };
            // Wide enough for the description, so it never spills out.
            let desc_w = ui
                .painter()
                .layout_no_wrap(
                    // The longer text sets both cards' width.
                    "Category home, sub-page pane, soft corners".to_owned(),
                    regular(11.5),
                    pal.text_faint,
                )
                .size()
                .x;
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new((desc_w + 36.0).max(240.0), 78.0), Sense::click());
            let fill = if resp.hovered() { pal.hover } else { pal.card };
            ui.painter().rect_filled(rect, pal.corner(8), fill);
            ui.painter().rect_stroke(
                rect,
                pal.corner(8),
                Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        pal.accent
                    } else {
                        pal.card_border
                    },
                ),
                StrokeKind::Inside,
            );
            let dot = Rect::from_min_size(rect.min + Vec2::new(16.0, 16.0), Vec2::splat(18.0));
            let preview_accent = if design == crate::theme::Design::Win10 {
                Color32::from_rgb(0x00, 0x78, 0xD7)
            } else {
                pal.accent
            };
            ui.painter().rect_filled(dot, pal.corner(4), preview_accent);
            ui.painter().text(
                Pos2::new(dot.right() + 10.0, dot.center().y),
                Align2::LEFT_CENTER,
                design.label(),
                semibold(14.0),
                pal.text,
            );
            ui.painter().text(
                Pos2::new(rect.left() + 16.0, rect.bottom() - 18.0),
                Align2::LEFT_CENTER,
                desc,
                regular(11.5),
                pal.text_faint,
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                chosen = design;
            }
        }
    });
    spacer(ui, 18.0);
    if chosen != app.design {
        app.set_design(chosen);
        return;
    }
    glass_section(app, ui, &pal);
    if app.design == crate::theme::Design::Win10 {
        widgets::card(ui, &pal, |ui| {
            let mut on = app.home_links;
            let outcome = widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Home,
                "Page links on the home cards",
                "List each category's pages under its name",
                Right::Switch(&mut on),
            );
            if outcome.toggled {
                app.set_home_links(on);
            }
        });
        spacer(ui, 18.0);
    }

    widgets::section_label(ui, &pal, "Choose your mode");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for choice in [Choice::Light, Choice::Dark, Choice::System] {
            let selected = match choice {
                Choice::Light => !app.follow_system && app.mode == crate::theme::Mode::Light,
                Choice::Dark => !app.follow_system && app.mode == crate::theme::Mode::Dark,
                Choice::System => app.follow_system,
            };
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(148.0, 100.0), Sense::click());
            let preview = match choice {
                Choice::Light => Color32::from_rgb(0xF3, 0xF3, 0xF3),
                Choice::Dark => Color32::from_rgb(0x20, 0x20, 0x20),
                Choice::System => {
                    if app.system_mode == crate::theme::Mode::Light {
                        Color32::from_rgb(0xF3, 0xF3, 0xF3)
                    } else {
                        Color32::from_rgb(0x20, 0x20, 0x20)
                    }
                }
            };
            ui.painter().rect_filled(rect, pal.corner(8), preview);
            ui.painter().rect_stroke(
                rect,
                pal.corner(8),
                Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        pal.accent
                    } else {
                        pal.card_border
                    },
                ),
                StrokeKind::Inside,
            );
            let (label, note) = match choice {
                Choice::Light => ("Light", "Bright surfaces"),
                Choice::Dark => ("Dark", "Low-light surfaces"),
                Choice::System => ("Use system", "Follow Ubuntu"),
            };
            let on_light = matches!(choice, Choice::Light)
                || (matches!(choice, Choice::System)
                    && app.system_mode == crate::theme::Mode::Light);
            let fg = if on_light {
                Color32::from_rgb(0x20, 0x20, 0x20)
            } else {
                Color32::WHITE
            };
            ui.painter().text(
                Pos2::new(rect.center().x, rect.bottom() - 34.0),
                Align2::CENTER_CENTER,
                label,
                semibold(13.0),
                fg,
            );
            ui.painter().text(
                Pos2::new(rect.center().x, rect.bottom() - 18.0),
                Align2::CENTER_CENTER,
                note,
                regular(11.0),
                fg.gamma_multiply(0.75),
            );
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            if resp.clicked() {
                match choice {
                    Choice::Light => {
                        app.follow_system = false;
                        app.mode = crate::theme::Mode::Light;
                    }
                    Choice::Dark => {
                        app.follow_system = false;
                        app.mode = crate::theme::Mode::Dark;
                    }
                    Choice::System => app.use_system_appearance(),
                }
            }
        }
    });

    spacer(ui, 18.0);
    widgets::section_label(ui, &pal, "Accent colour");
    widgets::card(ui, &pal, |ui| {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            ui.spacing_mut().item_spacing.x = 10.0;
            for (name, color) in crate::theme::ACCENTS {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
                let selected = app.accent == color && !app.follow_system;
                if pal.design == crate::theme::Design::Win10 {
                    let swatch = Rect::from_center_size(rect.center(), Vec2::splat(25.0));
                    ui.painter().rect_filled(swatch, 0, color);
                    if selected {
                        ui.painter().rect_stroke(
                            swatch.expand(3.0),
                            0,
                            Stroke::new(1.5, pal.text),
                            StrokeKind::Inside,
                        );
                    }
                } else {
                    ui.painter().circle_filled(rect.center(), 12.5, color);
                    if selected {
                        ui.painter()
                            .circle_stroke(rect.center(), 15.5, Stroke::new(2.0, pal.text));
                    }
                }
                let resp = resp
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(name);
                if resp.clicked() {
                    app.follow_system = false;
                    app.accent = color;
                    app.notify(format!("{name} accent applied"));
                }
            }

            // Custom colour: a native picker, so any accent is possible.
            ui.add_space(6.0);
            let before = app.accent;
            let mut rgb = [before.r(), before.g(), before.b()];
            let picker = ui.color_edit_button_srgb(&mut rgb);
            if picker.changed() {
                app.follow_system = false;
                app.accent = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            }
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
            if pal.design == crate::theme::Design::Win10 {
                ui.painter().rect_filled(
                    Rect::from_center_size(dot.center(), Vec2::splat(12.0)),
                    0,
                    app.accent,
                );
            } else {
                ui.painter().circle_filled(dot.center(), 6.0, app.accent);
            }
            ui.label(
                egui::RichText::new("Custom")
                    .font(regular(12.0))
                    .color(pal.text_muted),
            );
        });
        ui.add_space(16.0);
    });

    spacer(ui, 14.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        if widgets::button(ui, &pal, pal.accent, "Apply to Ubuntu", None).clicked() {
            app.apply_appearance_to_desktop();
        }
        if widgets::ghost_button(
            ui,
            &pal,
            if app.follow_system {
                "Re-sync with system"
            } else {
                "Use system colours"
            },
            Some(Icon::Display),
        )
        .clicked()
        {
            app.use_system_appearance();
        }
        if widgets::ghost_button(ui, &pal, "Show the welcome guide", Some(Icon::Sparkle)).clicked()
        {
            app.welcome = Some(crate::welcome::Welcome::new(app));
        }
    });
    note(
        ui,
        &pal,
        if app.follow_system {
            "Following Ubuntu: the mode and accent update automatically when the desktop changes."
        } else {
            "This app uses its own colours. “Apply to Ubuntu” also updates the desktop theme and accent."
        },
    );

    spacer(ui, 18.0);
    widgets::card(ui, &pal, |ui| {
        let wallpaper = shorten(
            app.info.wallpaper.rsplit('/').next().unwrap_or("Unknown"),
            46,
        );
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Image,
            "Wallpaper",
            &wallpaper,
            Right::AccentText("Change…"),
        )
        .clicked()
        {
            app.open_wallpaper_picker();
        }
        widgets::divider(ui, &pal);
        widgets::info_row(
            ui,
            &pal,
            "System accent",
            &format!(
                "{} · {}",
                capitalize(&app.system_accent_name),
                if app.system_mode == crate::theme::Mode::Dark {
                    "Dark"
                } else {
                    "Light"
                }
            ),
            Right::None,
        );
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Pointer");
    widgets::card(ui, &pal, |ui| pointer_size_row(app, ui, &pal));
}

fn pointer_size_row(app: &mut App, ui: &mut Ui, pal: &Palette) {
    let mut cursor = app.info.cursor_size as f32;
    if range_row(
        ui,
        pal,
        Icon::Pointer,
        "Pointer size",
        format!("{} px", cursor.round() as u32),
        &mut cursor,
        16.0,
        96.0,
        8.0,
    ) {
        let px = cursor.round() as u32;
        if system::set_cursor_size(px) {
            app.info.cursor_size = px;
        } else {
            app.notify_error("Could not change the pointer size");
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Light,
    Dark,
    System,
}

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

pub fn accessibility(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Accessibility");

    let mut apply_scale: Option<f32> = None;

    widgets::card(ui, &pal, |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let (chip, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
            widgets::icon_tile(
                ui.painter(),
                chip,
                Icon::Accessibility,
                pal.accent,
                pal.on_accent,
                &pal,
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    egui::RichText::new("Text size")
                        .font(regular(14.0))
                        .color(pal.text),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{:.0}% · applies to this app and Ubuntu apps",
                        app.text_scale * 100.0
                    ))
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD + 46.0);
            let width = ui.available_width() - ROW_PAD - 6.0;
            ui.allocate_ui_with_layout(
                Vec2::new(width, 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(width);
                    let mut value = ((app.text_scale - 1.0) / 0.5).clamp(0.0, 1.0);
                    if widgets::slider(ui, &pal, pal.accent, &mut value, Id::new("text-scale")) {
                        let scale = 1.0 + value * 0.5;
                        app.text_scale = scale;
                        apply_scale = Some(scale);
                    }
                },
            );
        });
        ui.add_space(14.0);
    });

    if let Some(scale) = apply_scale {
        ui.ctx().set_zoom_factor(scale * crate::app::APP_ZOOM);
        if !system::set_text_scale(scale) {
            app.notify_error("Could not change the Ubuntu text scale");
        }
        app.a11y.text_scale = scale;
    }

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Vision");
    widgets::card(ui, &pal, |ui| {
        let mut large = app.a11y.large_text || app.text_scale >= 1.15;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Display,
            "Large text",
            "Scale every interface up to 125%",
            Right::Switch(&mut large),
        );
        if outcome.toggled() {
            let scale = if large { 1.25 } else { 1.0 };
            app.a11y.large_text = large;
            app.text_scale = scale;
            ui.ctx().set_zoom_factor(scale * crate::app::APP_ZOOM);
            if system::set_text_scale(scale) {
                app.notify(if large {
                    "Large text turned on"
                } else {
                    "Large text turned off"
                });
            } else {
                app.notify_error("Could not change the text scale");
            }
        }
        widgets::divider(ui, &pal);
        let mut alerts = app.a11y.visual_alerts;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Bell,
            "Visual alerts",
            "Flash the screen instead of playing a system beep",
            Right::Switch(&mut alerts),
        );
        if outcome.toggled() {
            if system::set_visual_alerts(alerts) {
                app.a11y.visual_alerts = alerts;
                app.notify("Visual alerts updated");
            } else {
                app.notify_error("Could not change visual alerts");
            }
        }
        widgets::divider(ui, &pal);
        let mut animations = app.a11y.animations;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Sparkle,
            "Animations",
            "Turn off for a snappier, calmer interface",
            Right::Switch(&mut animations),
        );
        if outcome.toggled() {
            if system::set_animations(animations) {
                app.a11y.animations = animations;
                app.notify(if animations {
                    "Animations enabled"
                } else {
                    "Animations disabled"
                });
            } else {
                app.notify_error("Could not change animations");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Input");
    widgets::card(ui, &pal, |ui| {
        let mut sticky = app.a11y.sticky_keys;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Keyboard,
            "Sticky keys",
            "Press shortcuts one key at a time",
            Right::Switch(&mut sticky),
        );
        if outcome.toggled() {
            if system::set_sticky_keys(sticky) {
                app.a11y.sticky_keys = sticky;
                app.notify("Sticky keys updated");
            } else {
                app.notify_error("Could not change sticky keys");
            }
        }
        widgets::divider(ui, &pal);
        let mut mouse = app.a11y.mouse_keys;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::System,
            "Mouse keys",
            "Use the numeric keypad to move the pointer",
            Right::Switch(&mut mouse),
        );
        if outcome.toggled() {
            if system::set_mouse_keys(mouse) {
                app.a11y.mouse_keys = mouse;
                app.notify("Mouse keys updated");
            } else {
                app.notify_error("Could not change mouse keys");
            }
        }
    });
}

pub fn privacy(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Privacy & security");

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

pub fn time_language(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    widgets::h1(ui, &pal, "Time & language");

    if app.clock_refresh.elapsed().as_secs() >= 1 {
        app.info.local_time = system::local_time();
        app.clock_refresh = std::time::Instant::now();
    }

    widgets::card(ui, &pal, |ui| {
        widgets::info_row(ui, &pal, "Time zone", &app.info.timezone, Right::None);
        widgets::divider(ui, &pal);
        widgets::info_row(ui, &pal, "Local time", &app.info.local_time, Right::None);
        widgets::divider(ui, &pal);
        let mut ntp = app.info.ntp_enabled;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Globe,
            "Set time automatically",
            "Synchronise the clock with internet time servers",
            Right::Switch(&mut ntp),
        );
        if outcome.toggled {
            app.set_ntp(ntp);
        }
        widgets::divider(ui, &pal);
        let mut locales = system::list_locales();
        let current = if app.info.locale.is_empty() {
            "Not set".to_owned()
        } else {
            app.info.locale.clone()
        };
        if !locales.iter().any(|l| *l == current) && !app.info.locale.is_empty() {
            locales.insert(0, current.clone());
        }
        let locale_options: Vec<(String, String)> = locales
            .iter()
            .map(|l| {
                let base = l.split('.').next().unwrap_or(l);
                (
                    l.clone(),
                    base.replace('_', " (") + if base.contains('_') { ")" } else { "" },
                )
            })
            .collect();
        if let Some(locale) = combo_row(
            ui,
            &pal,
            Icon::Globe,
            "Language",
            "Applied the next time you sign in",
            "system-language",
            &locale_options,
            &current,
            200.0,
        ) {
            app.set_locale(locale);
        }
        widgets::divider(ui, &pal);
        let mut clock = app.info.clock_24h;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "24-hour clock",
            "Show 18:30 instead of 6:30 PM",
            Right::Switch(&mut clock),
        );
        if outcome.toggled {
            if system::set_clock_24h(clock) {
                app.info.clock_24h = clock;
                app.notify("Clock format updated");
            } else {
                app.notify_error("Could not change the clock format");
            }
        }
        widgets::divider(ui, &pal);
        let mut show_date = app.info.clock_show_date;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Show date",
            "Display the date next to the time",
            Right::Switch(&mut show_date),
        );
        if outcome.toggled {
            if system::set_clock_detail("clock-show-date", show_date) {
                app.info.clock_show_date = show_date;
            } else {
                app.notify_error("Could not change the top bar clock");
            }
        }
        widgets::divider(ui, &pal);
        let mut weekday = app.info.clock_show_weekday;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Show weekday",
            "Display the day of the week next to the time",
            Right::Switch(&mut weekday),
        );
        if outcome.toggled {
            if system::set_clock_detail("clock-show-weekday", weekday) {
                app.info.clock_show_weekday = weekday;
            } else {
                app.notify_error("Could not change the top bar clock");
            }
        }
        widgets::divider(ui, &pal);
        let mut seconds = app.info.clock_show_seconds;
        let outcome = widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Clock,
            "Show seconds",
            "Display seconds next to the time",
            Right::Switch(&mut seconds),
        );
        if outcome.toggled {
            if system::set_clock_detail("clock-show-seconds", seconds) {
                app.info.clock_show_seconds = seconds;
            } else {
                app.notify_error("Could not change the top bar clock");
            }
        }
    });

    spacer(ui, 16.0);
    widgets::section_label(ui, &pal, "Change time zone");
    let mut filter = app.timezone_filter.clone();
    let mut chosen = None;
    widgets::card(ui, &pal, |ui| {
        _ = widgets::search_box(
            ui,
            &pal,
            &mut filter,
            false,
            "Filter time zones, e.g. Berlin",
        );
        let query = filter.trim().to_lowercase();
        ui.add_space(6.0);
        let zones = system::list_timezones();
        let mut visible: Vec<&String> = zones
            .iter()
            .filter(|z| query.is_empty() || z.to_lowercase().contains(&query))
            .collect();
        if query.is_empty() {
            // Show the active zone first so it is visible without searching.
            if let Some(index) = visible.iter().position(|z| **z == app.info.timezone) {
                let current = visible.remove(index);
                visible.insert(0, current);
            }
        }
        let mut shown = 0usize;
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .id_salt("tz-list")
            .show(ui, |ui| {
                for zone in visible.iter().take(400) {
                    shown += 1;
                    let current = **zone == app.info.timezone;
                    if widgets::row(
                        ui,
                        &pal,
                        pal.accent,
                        Icon::Globe,
                        zone,
                        if current { "Current" } else { "" },
                        if current { Right::Check } else { Right::None },
                    )
                    .clicked()
                        && !current
                    {
                        chosen = Some((*zone).clone());
                    }
                }
                if shown == 0 {
                    widgets::empty_note(ui, &pal, "No time zone matches that filter.");
                }
            });
    });
    if filter != app.timezone_filter {
        app.timezone_filter = filter;
    }
    if let Some(zone) = chosen {
        app.set_timezone(zone);
    }
}

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

/// Suffix for catalogue rows: App Hub's own error text or a stale marker.
fn hub_note(item: &HubApp) -> String {
    if let Some(error) = &item.error {
        format!(" · {error}")
    } else if item.stale {
        " · cached version".to_owned()
    } else {
        String::new()
    }
}

// ------------------------------------------------ application management

/// Outcome of one of the small text buttons on a hub row.
fn hub_row(
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
enum SysAppAction {
    Open,
    Uninstall,
}

/// A row in the “All installed applications” inventory — like App Hub’s
/// All Apps page: real icon, version · source · size, channel chip, and
/// Open/Uninstall actions. Protected entries show no destructive action.
fn sys_app_row(
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

// ---------------------------------------------------------------- tabs
// Windows-11 style tab strip for the Apps page: My Apps / Updates / Library /
// All Apps, badged like App Hub's own bar. The catalogue tabs hide without
// App Hub; the inventory and Ubuntu packages work standalone.

fn apps_tab_bar(app: &mut App, ui: &mut Ui) {
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

fn apps_tab_button(
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

fn render_my_apps_tab(
    app: &mut App,
    ui: &mut Ui,
    pending: &mut Option<(crate::hub::Action, String)>,
    confirm_remove: &mut Option<(String, String)>,
    update_all: &mut bool,
) {
    let pal = app.pal();

    // ------------------------------------------------------------ hero
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

    // ------------------------------------------------------------ filter
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

    // ------------------------------------------------------------ installed
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

    // ------------------------------------------------------------ catalogue
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

fn render_updates_tab(
    app: &mut App,
    ui: &mut Ui,
    pending: &mut Option<(crate::hub::Action, String)>,
    confirm_remove: &mut Option<(String, String)>,
) {
    let pal = app.pal();

    // ------------------------------------------------------------ Ubuntu packages
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

    // ------------------------------------------------------------ App Hub updates
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

fn render_library_tab(
    app: &mut App,
    ui: &mut Ui,
    pending: &mut Option<(crate::hub::Action, String)>,
) {
    let pal = app.pal();

    // ------------------------------------------------------------ add app
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

    // ------------------------------------------------------------ filter
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
fn render_favorites(app: &mut App, ui: &mut Ui) {
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
enum LibraryRowAction {
    Primary,
    Remove,
}

fn library_row(
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

fn render_all_apps_tab(
    app: &mut App,
    ui: &mut Ui,
    remove_sys: &mut Option<crate::inventory::SysApp>,
    launch_desktop: &mut Option<String>,
) {
    let pal = app.pal();

    // ------------------------------------------------------------ filter
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

fn app_hub_controls(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let mut pending: Option<(crate::hub::Action, String)> = None;
    let mut confirm_remove: Option<(String, String)> = None;
    let mut remove_sys: Option<crate::inventory::SysApp> = None;
    let mut launch_desktop: Option<String> = None;
    let mut update_all = false;

    apps_tab_bar(app, ui);
    spacer(ui, 12.0);

    // ------------------------------------------------------------ running job
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

// ---------------------------------------------------------------- search

pub struct SearchEntry {
    pub page: Page,
    pub detail: Option<Detail>,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub icon: Icon,
}

pub fn search_entries() -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for page in Page::ALL {
        entries.push(SearchEntry {
            page,
            detail: None,
            title: page.label(),
            subtitle: "Settings page",
            icon: page.icon(),
        });
    }
    let details = [
        (
            Detail::Display,
            "Resolution, brightness, night light",
            Icon::Display,
        ),
        (Detail::Sound, "Volume, output devices, mute", Icon::Sound),
        (
            Detail::Notifications,
            "Banners, lock screen alerts",
            Icon::Bell,
        ),
        (
            Detail::Power,
            "Power mode, battery, screen timeout",
            Icon::Battery,
        ),
        (Detail::Storage, "Disk space and volumes", Icon::Storage),
        (
            Detail::About,
            "Device specifications and rename",
            Icon::Info,
        ),
    ];
    for (detail, subtitle, icon) in details {
        entries.push(SearchEntry {
            page: Page::System,
            detail: Some(detail),
            title: detail.title(),
            subtitle,
            icon,
        });
    }
    entries.push(SearchEntry {
        page: Page::Personalisation,
        detail: None,
        title: "Accent colour",
        subtitle: "Change the highlight colour",
        icon: Icon::Personalise,
    });
    entries.push(SearchEntry {
        page: Page::Personalisation,
        detail: None,
        title: "Dark mode",
        subtitle: "Light or dark appearance",
        icon: Icon::Moon,
    });
    entries.push(SearchEntry {
        page: Page::TimeLanguage,
        detail: None,
        title: "Date & time",
        subtitle: "Time zone, clock format",
        icon: Icon::Clock,
    });
    for (page, title, subtitle, icon) in [
        (
            Page::Desktop,
            "Multitasking",
            "Hot corner, screen edges, workspaces, Alt+Tab",
            Icon::Apps,
        ),
        (
            Page::Desktop,
            "Ubuntu Dock",
            "Auto-hide, panel mode, icon size, position",
            Icon::Apps,
        ),
        (
            Page::Desktop,
            "Desktop icons",
            "Home folder, Trash, icon size",
            Icon::Image,
        ),
        (
            Page::Desktop,
            "Title bar buttons",
            "Minimize, maximize, window focus, title bar clicks",
            Icon::Apps,
        ),
        (
            Page::Desktop,
            "Fonts",
            "Interface, document and monospace fonts, hinting",
            Icon::Keyboard,
        ),
        (
            Page::Desktop,
            "Themes",
            "Legacy app theme, icons, cursor",
            Icon::Personalise,
        ),
        (
            Page::Desktop,
            "Startup apps",
            "Apps that start when you sign in",
            Icon::Power,
        ),
        (
            Page::Desktop,
            "Tweaks",
            "Overview key, middle-click paste, mouse acceleration, over-amplification",
            Icon::Sparkle,
        ),
        (
            Page::Extensions,
            "GNOME extensions",
            "Turn on or off, settings, install, remove",
            Icon::Hub,
        ),
    ] {
        entries.push(SearchEntry {
            page,
            detail: None,
            title,
            subtitle,
            icon,
        });
    }
    entries.push(SearchEntry {
        page: Page::Updates,
        detail: None,
        title: "Windows Update",
        subtitle: "Check for and install package updates",
        icon: Icon::Update,
    });
    entries
}

pub fn search(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let query = app.search.trim().to_lowercase();
    widgets::h1(ui, &pal, "Search results");
    ui.label(
        egui::RichText::new(format!("Results for “{}”", app.search.trim()))
            .font(regular(12.5))
            .color(pal.text_faint),
    );
    spacer(ui, 14.0);

    let entries = search_entries();
    let matches = entries
        .into_iter()
        .filter(|e| {
            e.title.to_lowercase().contains(&query)
                || e.subtitle.to_lowercase().contains(&query)
                || e.page.label().to_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "No settings match that search. Try “display”, “sound” or “storage”.",
            );
        });
        return;
    }

    let mut target: Option<View> = None;
    widgets::card(ui, &pal, |ui| {
        for (index, entry) in matches.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let subtitle = format!("{} · {}", entry.page.label(), entry.subtitle);
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                entry.icon,
                entry.title,
                &subtitle,
                Right::Chevron,
            )
            .clicked()
            {
                target = Some(match entry.detail {
                    Some(detail) => View::Detail(entry.page, detail),
                    None => View::Page(entry.page),
                });
            }
        }
    });
    if let Some(view) = target {
        app.go(view);
    }
}

// ---------------------------------------------------------------- home (Win10)

/// A toggle on the Windows 10 home page.
#[derive(Clone, Copy)]
enum Quick {
    Wifi,
    Bluetooth,
    NightLight,
    Focus,
    Mute,
    Dark,
}

impl Quick {
    const ALL: [Quick; 6] = [
        Quick::Wifi,
        Quick::Bluetooth,
        Quick::NightLight,
        Quick::Focus,
        Quick::Mute,
        Quick::Dark,
    ];

    fn label(self) -> &'static str {
        match self {
            Quick::Wifi => "Wi-Fi",
            Quick::Bluetooth => "Bluetooth",
            Quick::NightLight => "Night light",
            Quick::Focus => "Do not disturb",
            Quick::Mute => "Mute",
            Quick::Dark => "Dark mode",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Quick::Wifi => Icon::Wifi,
            Quick::Bluetooth => Icon::Bluetooth,
            Quick::NightLight => Icon::Sun,
            Quick::Focus => Icon::Bell,
            Quick::Mute => Icon::Sound,
            Quick::Dark => Icon::Moon,
        }
    }

    fn is_on(self, app: &App) -> bool {
        match self {
            Quick::Wifi => app.info.wifi_enabled,
            Quick::Bluetooth => app.info.bluetooth_on,
            Quick::NightLight => app.info.night_light,
            Quick::Focus => !app.info.notifications,
            Quick::Mute => app.info.muted,
            Quick::Dark => app.mode == crate::theme::Mode::Dark,
        }
    }

    fn toggle(self, app: &mut App) {
        let on = !self.is_on(app);
        let state = if on { "on" } else { "off" };
        let ok = match self {
            Quick::Wifi => {
                app.set_wifi_radio(on);
                return;
            }
            Quick::Bluetooth => match system::set_bluetooth(on) {
                Ok(()) => {
                    app.info.bluetooth_on = on;
                    true
                }
                Err(err) => {
                    app.notify_error(err);
                    return;
                }
            },
            Quick::NightLight => {
                let ok = system::set_night_light(on);
                if ok {
                    app.info.night_light = on;
                }
                ok
            }
            Quick::Focus => {
                let ok = system::set_notifications(!on);
                if ok {
                    app.info.notifications = !on;
                }
                ok
            }
            Quick::Mute => {
                let ok = system::set_muted(on);
                if ok {
                    app.info.muted = on;
                }
                ok
            }
            Quick::Dark => {
                app.follow_system = false;
                app.mode = if on {
                    crate::theme::Mode::Dark
                } else {
                    crate::theme::Mode::Light
                };
                return;
            }
        };
        if ok {
            app.notify(format!("{} turned {state}", self.label()));
        } else {
            app.notify_error(format!("Could not change {}", self.label().to_lowercase()));
        }
    }
}

/// Extra home-page links for categories with few sub-pages: the main
/// settings on that page, so every card shows what it holds.
fn home_extras(cat: crate::app::Cat10) -> &'static [(&'static str, Icon, View)] {
    use crate::app::Cat10;
    match cat {
        Cat10::Network => &[
            ("Wi-Fi networks", Icon::Wifi, View::Page(Page::Network)),
            ("Connected network", Icon::Globe, View::Page(Page::Network)),
            ("Proxy", Icon::Shield, View::Page(Page::Network)),
        ],
        Cat10::Accounts => &[
            ("Password", Icon::Shield, View::Page(Page::Accounts)),
            ("Automatic sign-in", Icon::Power, View::Page(Page::Accounts)),
            ("Lock screen", Icon::Display, View::Page(Page::Privacy)),
        ],
        Cat10::Time => &[
            (
                "Set time automatically",
                Icon::Clock,
                View::Page(Page::TimeLanguage),
            ),
            ("Language", Icon::Globe, View::Page(Page::TimeLanguage)),
            ("Clock format", Icon::Clock, View::Page(Page::TimeLanguage)),
        ],
        Cat10::Ease => &[
            ("Large text", Icon::Display, View::Page(Page::Accessibility)),
            ("Visual alerts", Icon::Bell, View::Page(Page::Accessibility)),
            (
                "Sticky keys",
                Icon::Keyboard,
                View::Page(Page::Accessibility),
            ),
        ],
        Cat10::Privacy => &[
            ("Screen lock", Icon::Display, View::Page(Page::Privacy)),
            ("Location services", Icon::Globe, View::Page(Page::Privacy)),
            (
                "Camera & microphone",
                Icon::Image,
                View::Page(Page::Privacy),
            ),
        ],
        Cat10::Update => &[
            ("Available updates", Icon::Update, View::Page(Page::Updates)),
            ("Update policy", Icon::Clock, View::Page(Page::Updates)),
            ("App Hub apps", Icon::Hub, View::Page(Page::Updates)),
        ],
        Cat10::Personalisation => &[(
            "Light & dark mode",
            Icon::Moon,
            View::Page(Page::Personalisation),
        )],
        Cat10::Apps => &[("App Hub", Icon::Hub, View::Page(Page::Apps))],
        _ => &[],
    }
}

/// Quick toggles (Wi-Fi, Bluetooth, night light, do not disturb, mute, dark
/// mode) and the volume and brightness sliders. Shared by the Windows 10 home
/// and the Windows 11 System page.
pub fn quick_settings(
    app: &mut App,
    ui: &mut Ui,
    pal: &Palette,
    width: f32,
    x0: f32,
    content_w: f32,
    s: f32,
) {
    let cols = if content_w >= 6.0 * 150.0 { 6 } else { 3 };
    let gap = 10.0 * s;
    let qw = (content_w - gap * (cols as f32 - 1.0)) / cols as f32;
    let qh = 74.0 * s;
    let rows = Quick::ALL.len().div_ceil(cols);
    let (area, _) = ui.allocate_exact_size(
        Vec2::new(width, rows as f32 * qh + (rows as f32 - 1.0) * gap),
        Sense::hover(),
    );
    let mut toggled: Option<Quick> = None;
    for (i, quick) in Quick::ALL.into_iter().enumerate() {
        let tile = Rect::from_min_size(
            Pos2::new(
                x0 + (i % cols) as f32 * (qw + gap),
                area.top() + (i / cols) as f32 * (qh + gap),
            ),
            Vec2::new(qw, qh),
        );
        let on = quick.is_on(app);
        let resp = ui
            .interact(tile, Id::new(("home-toggle", i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let (fill, ink, sub) = if on {
            let fill = if resp.hovered() {
                crate::theme::lighten(pal.accent, 14)
            } else {
                pal.accent
            };
            (fill, pal.on_accent, pal.on_accent.gamma_multiply(0.85))
        } else {
            let fill = if resp.hovered() {
                pal.card_hover
            } else {
                pal.card
            };
            (fill, pal.text, pal.text_faint)
        };
        ui.painter()
            .add(widgets::glass_fill(pal, tile, pal.corner(6), fill));
        if !on && !widgets::glassy(pal) {
            ui.painter().rect_stroke(
                tile,
                pal.corner(6),
                Stroke::new(1.0, pal.card_border),
                StrokeKind::Inside,
            );
        }
        let glyph = Rect::from_min_size(
            tile.min + Vec2::new(14.0 * s, 14.0 * s),
            Vec2::splat(20.0 * s),
        );
        crate::icons::paint(ui.painter(), quick.icon(), glyph, ink);
        let painter = ui.painter().with_clip_rect(tile.shrink(4.0));
        painter.text(
            Pos2::new(tile.left() + 14.0 * s, tile.bottom() - 26.0 * s),
            Align2::LEFT_CENTER,
            quick.label(),
            semibold(12.5 * s),
            ink,
        );
        painter.text(
            Pos2::new(tile.left() + 14.0 * s, tile.bottom() - 11.0 * s),
            Align2::LEFT_CENTER,
            if on { "On" } else { "Off" },
            regular(11.0 * s),
            sub,
        );
        if resp.clicked() {
            toggled = Some(quick);
        }
    }
    if let Some(quick) = toggled {
        quick.toggle(app);
    }

    ui.add_space(gap);
    let brightness = app.info.brightness;
    let sliders = if brightness.is_some() { 2 } else { 1 };
    let sw = (content_w - gap * (sliders as f32 - 1.0)) / sliders as f32;
    let sh = 62.0 * s;
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, sh), Sense::hover());
    for i in 0..sliders {
        let card = Rect::from_min_size(
            Pos2::new(x0 + i as f32 * (sw + gap), area.top()),
            Vec2::new(sw, sh),
        );
        ui.painter()
            .add(widgets::glass_fill(pal, card, pal.corner(6), pal.card));
        ui.painter().rect_stroke(
            card,
            pal.corner(6),
            Stroke::new(1.0, pal.card_border),
            StrokeKind::Inside,
        );
        let (icon, label, mut value, readout) = if i == 0 {
            let v = app.info.volume as f32 / 100.0;
            let text = if app.info.muted {
                "Muted".to_owned()
            } else {
                format!("{}%", app.info.volume)
            };
            (Icon::Sound, "Volume", v, text)
        } else {
            let pct = brightness.unwrap_or(100);
            (
                Icon::Display,
                "Brightness",
                pct as f32 / 100.0,
                format!("{pct}%"),
            )
        };
        let glyph = Rect::from_center_size(
            Pos2::new(card.left() + 26.0 * s, card.center().y),
            Vec2::splat(20.0 * s),
        );
        crate::icons::paint(ui.painter(), icon, glyph, pal.accent);
        ui.painter().text(
            Pos2::new(card.left() + 48.0 * s, card.top() + 18.0 * s),
            Align2::LEFT_CENTER,
            label,
            semibold(12.5 * s),
            pal.text,
        );
        ui.painter().text(
            Pos2::new(card.right() - 18.0 * s, card.top() + 18.0 * s),
            Align2::RIGHT_CENTER,
            readout,
            regular(12.0 * s),
            pal.text_muted,
        );
        let track = Rect::from_min_max(
            Pos2::new(card.left() + 48.0 * s, card.top() + 28.0 * s),
            Pos2::new(card.right() - 18.0 * s, card.bottom() - 4.0 * s),
        );
        let changed = ui
            .scope_builder(UiBuilder::new().max_rect(track), |ui| {
                widgets::slider(ui, pal, pal.accent, &mut value, Id::new(("home-slider", i)))
            })
            .inner;
        if changed {
            if i == 0 {
                let pct = (value * 100.0).round() as u32;
                if pct != app.info.volume && system::set_volume(pct) {
                    app.info.volume = pct;
                    app.test_volume = value;
                    if app.info.muted && system::set_muted(false) {
                        app.info.muted = false;
                    }
                }
            } else {
                let pct = (value * 100.0).round().max(1.0) as u8;
                if Some(pct) != app.info.brightness {
                    match system::set_brightness(pct) {
                        Ok(()) => {
                            app.test_brightness = value;
                            app.info.brightness = Some(pct);
                        }
                        Err(err) => app.notify_error(err),
                    }
                }
            }
        }
    }
}

/// The Windows 10 home screen: an account card with shortcuts, the search
/// box, quick toggles and sliders, then every category as a card of links.
pub fn home10(app: &mut App, ui: &mut Ui, rect: Rect, pal: &Palette) {
    let width = rect.width();
    ScrollArea::vertical()
        .id_salt("home10")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // Laid out like the Windows 10 Settings start page: a grey band
            // across the whole window with the account and status items,
            // a centred search box, then every category as a large icon with
            // its title and description, three to a row, on the plain page.
            let s = (width / 1000.0).clamp(1.0, 1.25);
            let col_w = (width - 64.0 * s).min(1120.0 * s).max(300.0);
            let left = ui.cursor().left();
            let x0 = left + (width - col_w) / 2.0;
            let x1 = x0 + col_w;
            let mut go: Option<View> = None;

            // --- header band ----------------------------------------------
            let band_h = 168.0 * s;
            let (band, _) = ui.allocate_exact_size(Vec2::new(width, band_h), Sense::hover());
            let (top_c, bottom_c) = widgets::home10_header(pal);
            widgets::vgradient(ui.painter(), band, top_c, bottom_c);

            let account = app.info.full_name.clone();
            let role = if app.info.is_admin {
                "Local account · Administrator"
            } else {
                "Local account · Standard user"
            };
            let r = 46.0 * s;
            let avatar_c = Pos2::new(x0 + r, band.center().y);
            ui.painter().circle_stroke(
                avatar_c,
                r + 3.0 * s,
                Stroke::new(2.0 * s, with_alpha(pal.accent, 70)),
            );
            if let Some(tex) = widgets::user_photo(ui.ctx()) {
                widgets::paint_photo(ui.painter(), tex, avatar_c, r);
            } else {
                ui.painter().circle_filled(avatar_c, r, pal.accent);
                let initial = account
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_default();
                ui.painter().text(
                    avatar_c,
                    Align2::CENTER_CENTER,
                    initial,
                    semibold(36.0 * s),
                    pal.on_accent,
                );
            }
            // Click the picture to change it.
            let photo_resp = ui
                .interact(
                    Rect::from_center_size(avatar_c, Vec2::splat(r * 2.0)),
                    Id::new("home-photo"),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Change your picture");
            if photo_resp.hovered() {
                ui.painter()
                    .circle_filled(avatar_c, r, Color32::from_black_alpha(110));
                let cam = Rect::from_center_size(
                    Pos2::new(avatar_c.x, avatar_c.y - 8.0 * s),
                    Vec2::splat(24.0 * s),
                );
                crate::icons::paint(ui.painter(), Icon::Image, cam, Color32::WHITE);
                ui.painter().text(
                    Pos2::new(avatar_c.x, avatar_c.y + 18.0 * s),
                    Align2::CENTER_CENTER,
                    "Change",
                    regular(12.0 * s),
                    Color32::WHITE,
                );
            }
            if photo_resp.clicked() {
                app.load_photo(Some("pick"));
            }
            let name_x = avatar_c.x + r + 22.0 * s;
            let name_galley =
                ui.painter()
                    .layout_no_wrap(account.clone(), semibold(27.0 * s), pal.text);
            let name_w = name_galley.size().x;
            ui.painter().galley(
                Pos2::new(name_x, band.center().y - 30.0 * s),
                name_galley,
                pal.text,
            );
            ui.painter().text(
                Pos2::new(name_x, band.center().y + 10.0 * s),
                Align2::LEFT_CENTER,
                role,
                regular(13.0 * s),
                pal.text_muted,
            );
            let link = ui.painter().layout_no_wrap(
                "Manage your account".to_owned(),
                regular(13.0 * s),
                pal.accent,
            );
            let link_rect =
                Rect::from_min_size(Pos2::new(name_x, band.center().y + 22.0 * s), link.size());
            let link_resp = ui
                .interact(
                    link_rect.expand(3.0),
                    Id::new("home-account"),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            if link_resp.hovered() {
                ui.painter().line_segment(
                    [
                        Pos2::new(link_rect.left(), link_rect.bottom() + 1.0),
                        Pos2::new(link_rect.right(), link_rect.bottom() + 1.0),
                    ],
                    Stroke::new(1.0, pal.accent),
                );
            }
            ui.painter().galley(link_rect.min, link, pal.accent);
            if link_resp.clicked() {
                go = Some(View::Page(Page::Accounts));
            }

            // Status items on the right, two columns of two like Windows 10.
            let update_line = if app.updates_loading {
                "Checking…".to_owned()
            } else if let Some(list) = &app.info.updates {
                match list.len() {
                    0 => "You're up to date".to_owned(),
                    1 => "1 update available".to_owned(),
                    n => format!("{n} updates available"),
                }
            } else {
                "Check for updates".to_owned()
            };
            let net_line = app
                .info
                .wifi_networks
                .iter()
                .find(|n| n.active)
                .map(|n| format!("Connected to {}", n.ssid))
                .unwrap_or_else(|| "Wi-Fi, wired, proxy".to_owned());
            let status = [
                (
                    "Windows Update",
                    update_line,
                    Icon::Update,
                    View::Page(Page::Updates),
                ),
                (
                    "App Hub",
                    "Get and manage apps".to_owned(),
                    Icon::Hub,
                    View::Page(Page::Apps),
                ),
                ("Network", net_line, Icon::Wifi, View::Page(Page::Network)),
                (
                    "Storage",
                    format!(
                        "{:.0} GB free of {:.0} GB",
                        app.info.disk_free_gb, app.info.disk_total_gb
                    ),
                    Icon::Storage,
                    View::Detail(Page::System, Detail::Storage),
                ),
            ];
            // Status items a little smaller than the profile.
            let k = s * 0.88;
            let item_w = 230.0 * k;
            let room = x1 - (name_x + name_w.max(230.0 * k) + 30.0 * k);
            let cols = if room >= item_w * 2.0 {
                2
            } else if room >= item_w {
                1
            } else {
                0
            };
            let item_h = 54.0 * k;
            let shown = (cols * 2).min(status.len());
            let rows_shown = if cols == 0 { 0 } else { shown.div_ceil(cols) };
            let block_h =
                rows_shown as f32 * item_h + (rows_shown as f32 - 1.0).max(0.0) * 10.0 * k;
            let top = band.center().y - block_h / 2.0;
            for (i, (title, line, icon, view)) in status.into_iter().take(shown).enumerate() {
                let (row, col) = (i / cols.max(1), i % cols.max(1));
                let cell = Rect::from_min_size(
                    Pos2::new(
                        x1 - (cols - col) as f32 * item_w,
                        top + row as f32 * (item_h + 10.0 * k),
                    ),
                    Vec2::new(item_w - 8.0 * k, item_h),
                );
                let resp = ui
                    .interact(cell, Id::new(("home-status", i)), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                // Plain items on the band: a fill only while hovered.
                if resp.hovered() {
                    ui.painter().rect_filled(cell, pal.corner(4), pal.hover);
                }
                let glyph = Rect::from_center_size(
                    Pos2::new(cell.left() + 22.0 * k, cell.center().y),
                    Vec2::splat(28.0 * k),
                );
                crate::icons::paint(ui.painter(), icon, glyph, pal.text);
                let painter = ui.painter().with_clip_rect(cell);
                let tx = cell.left() + 50.0 * k;
                painter.text(
                    Pos2::new(tx, cell.center().y - 9.0 * k),
                    Align2::LEFT_CENTER,
                    title,
                    regular(14.5 * k),
                    pal.text,
                );
                painter.text(
                    Pos2::new(tx, cell.center().y + 10.0 * k),
                    Align2::LEFT_CENTER,
                    &line,
                    regular(12.0 * k),
                    pal.text_faint,
                );
                if resp.clicked() {
                    go = Some(view);
                }
            }

            // --- search ---------------------------------------------------
            ui.add_space(30.0 * s);
            let search_w = col_w.min(360.0 * s);
            let (row, _) = ui.allocate_exact_size(Vec2::new(width, 34.0 * s), Sense::hover());
            let search_rect = Rect::from_center_size(
                Pos2::new((x0 + x1) / 2.0, row.center().y),
                Vec2::new(search_w, 34.0 * s),
            );
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(search_rect)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.set_width(search_w);
                    let focus = std::mem::take(&mut app.focus_search);
                    widgets::search_box(ui, pal, &mut app.search, focus, "Find a setting");
                },
            );

            // --- page-links switch, right-aligned above the grid ----------
            ui.add_space(22.0 * s);
            {
                let label = if app.home_links {
                    "Hide page links"
                } else {
                    "Show page links"
                };
                let (row, _) = ui.allocate_exact_size(Vec2::new(width, 20.0 * s), Sense::hover());
                let galley =
                    ui.painter()
                        .layout_no_wrap(label.to_owned(), regular(12.5 * s), pal.accent);
                let pos = Pos2::new(x1 - galley.size().x, row.center().y - galley.size().y / 2.0);
                let hit = Rect::from_min_size(pos, galley.size()).expand(5.0);
                let resp = ui
                    .interact(hit, Id::new("home-links-toggle"), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                if resp.hovered() {
                    ui.painter().rect_filled(hit, pal.corner(4), pal.hover);
                }
                ui.painter().galley(pos, galley, pal.accent);
                if resp.clicked() {
                    let on = !app.home_links;
                    app.set_home_links(on);
                }
            }
            ui.add_space(8.0 * s);

            // Categories a little smaller than the profile and search.
            let k = s * 0.88;
            // --- every category: big icon, title, description -------------
            let show_links = app.home_links;
            let gap_x = 24.0 * k;
            let gap_y = if show_links { 26.0 * k } else { 30.0 * k };
            let min_w = 250.0 * k;
            let cols = (((col_w + gap_x) / (min_w + gap_x)).floor() as usize).clamp(1, 3);
            let cw = (col_w - gap_x * (cols as f32 - 1.0)) / cols as f32;
            let icon_sz = 34.0 * k;
            let text_x = 16.0 * k + icon_sz + 16.0 * k;
            let text_w = cw - text_x - 12.0 * k;
            let link_h = 23.0 * k;
            let max_links = 4usize;
            let head_h = 74.0 * k;
            let ch = if show_links {
                head_h + link_h * max_links as f32 + 6.0 * k
            } else {
                head_h
            };
            let cats = crate::app::Cat10::ALL;
            let rows = cats.len().div_ceil(cols);
            let (area, _) = ui.allocate_exact_size(
                Vec2::new(width, rows as f32 * ch + (rows as f32 - 1.0) * gap_y),
                Sense::hover(),
            );
            for (index, cat) in cats.iter().copied().enumerate() {
                let cell = Rect::from_min_size(
                    Pos2::new(
                        x0 + (index % cols) as f32 * (cw + gap_x),
                        area.top() + (index / cols) as f32 * (ch + gap_y),
                    ),
                    Vec2::new(cw, ch),
                );
                let head = Rect::from_min_size(cell.min, Vec2::new(cw, head_h));
                let resp = ui
                    .interact(
                        head,
                        Id::new(("home-category", cat.title())),
                        Sense::click(),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                if widgets::glassy(pal) {
                    ui.painter().add(widgets::glass_slab(
                        pal,
                        cell.expand2(Vec2::new(0.0, 4.0 * k)),
                        pal.corner(8).max(8),
                        resp.hovered(),
                    ));
                } else if resp.hovered() {
                    ui.painter().rect_filled(head, pal.corner(4), pal.hover);
                }
                let glyph = Rect::from_min_size(
                    Pos2::new(cell.left() + 16.0 * k, head.top() + 12.0 * k),
                    Vec2::splat(icon_sz),
                );
                crate::icons::paint(ui.painter(), cat.icon(), glyph, pal.accent);
                let painter = ui.painter().with_clip_rect(cell);
                let tx = cell.left() + text_x;
                painter.text(
                    Pos2::new(tx, head.top() + 12.0 * k),
                    Align2::LEFT_TOP,
                    cat.title(),
                    regular(15.5 * k),
                    pal.text,
                );
                let desc = painter.layout(
                    cat.subtitle().to_owned(),
                    regular(12.5 * k),
                    pal.text_faint,
                    text_w,
                );
                painter.galley(Pos2::new(tx, head.top() + 36.0 * k), desc, pal.text_faint);
                if resp.clicked() {
                    go = Some(cat.items()[0].2);
                }
                if !show_links {
                    continue;
                }

                let mut items: Vec<(&str, Icon, View)> = cat.items().to_vec();
                items.extend_from_slice(home_extras(cat));
                let overflow = items.len() > max_links;
                let shown = if overflow { max_links - 1 } else { items.len() };
                let mut links: Vec<(String, View)> = items[..shown]
                    .iter()
                    .map(|(label, _, view)| ((*label).to_owned(), *view))
                    .collect();
                if overflow {
                    links.push((format!("{} more…", items.len() - shown), items[shown].2));
                }
                for (j, (label, view)) in links.into_iter().enumerate() {
                    let galley = painter.layout_no_wrap(label, regular(13.0 * k), pal.accent);
                    let pos = Pos2::new(
                        tx,
                        head.bottom() + j as f32 * link_h + (link_h - galley.size().y) / 2.0,
                    );
                    let hit = Rect::from_min_size(pos, galley.size()).expand2(Vec2::new(4.0, 3.0));
                    let resp = ui
                        .interact(hit, Id::new(("home-link", index, j)), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand);
                    let color = if resp.hovered() { pal.text } else { pal.accent };
                    if resp.hovered() {
                        painter.line_segment(
                            [
                                Pos2::new(pos.x, pos.y + galley.size().y + 1.0),
                                Pos2::new(pos.x + galley.size().x, pos.y + galley.size().y + 1.0),
                            ],
                            Stroke::new(1.0, color),
                        );
                    }
                    painter.galley(pos, galley, color);
                    if resp.clicked() {
                        go = Some(view);
                    }
                }
            }

            // --- quick settings below the categories ----------------------
            ui.add_space(40.0 * s);
            let (row, _) = ui.allocate_exact_size(Vec2::new(width, 22.0 * s), Sense::hover());
            ui.painter().text(
                Pos2::new(x0 + 2.0, row.center().y),
                Align2::LEFT_CENTER,
                "Quick settings",
                semibold(16.0 * s),
                pal.text,
            );
            ui.add_space(12.0 * s);
            quick_settings(app, ui, pal, width, x0, col_w, s);

            if let Some(view) = go {
                app.go(view);
            }
            ui.add_space(36.0);
        });
}

/// The running or last finished update job, drawn App Hub-style inside the
/// updates card. Returns false when there is no job to show.
fn apt_job_panel(app: &mut App, ui: &mut Ui, pal: &Palette) -> bool {
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

/// Personalisation › Glass effects: Fusion Desk's glass settings for the
/// whole desktop ("Global"), for every Harbor app, and for each app. A
/// scope can follow the one above it or have its own values.
/// Ready-made glass looks (Settings › Personalisation › Glass effects).
const GLASS_PRESETS: [(&str, &str); 12] = [
    // The owner's Fusion Desk setup of 2026-11-19.
    ("Fusion Desk", "effect=SOFT,material=GLOSS,style=FROSTED,opacity=40,rim=true,glare=true,shadow=false,light-follows=false,style-strength=100,shine-strength=100,rim-strength=100,glare-strength=100,shadow-strength=100,light-strength=100"),
    ("Liquid", "effect=STRONG,material=GLOSS,style=CLEAR,opacity=55,rim=true,glare=true,shadow=true,light-follows=true,style-strength=100,shine-strength=80,rim-strength=160,glare-strength=120,shadow-strength=70,light-strength=130"),
    ("Frosted", "effect=STRONG,material=GLASS,style=FROSTED,opacity=75,rim=true,glare=true,shadow=true,light-follows=false,style-strength=120,shine-strength=100,rim-strength=100,glare-strength=80,shadow-strength=80,light-strength=100"),
    ("Glow", "effect=STRONG,material=GLASS,style=GLOW,opacity=60,rim=true,glare=false,shadow=true,light-follows=true,style-strength=110,shine-strength=100,rim-strength=120,glare-strength=100,shadow-strength=100,light-strength=100"),
    ("Smoke", "effect=STRONG,material=GLASS,style=SMOKE,opacity=70,rim=true,glare=true,shadow=true,light-follows=false,style-strength=100,shine-strength=100,rim-strength=90,glare-strength=70,shadow-strength=120,light-strength=100"),
    ("Aqua", "effect=STRONG,material=GLOSS,style=GEL,opacity=85,rim=true,glare=true,shadow=true,light-follows=true,style-strength=110,shine-strength=140,rim-strength=130,glare-strength=130,shadow-strength=90,light-strength=120"),
    ("Subtle", "effect=SOFT,material=GLASS,style=FROSTED,opacity=90,rim=true,glare=false,shadow=false,light-follows=false,style-strength=70,shine-strength=60,rim-strength=70,glare-strength=100,shadow-strength=100,light-strength=100"),
    ("Acrylic", "effect=STRONG,material=GLASS,style=FROSTED,opacity=70,rim=true,glare=false,shadow=true,light-follows=false,style-strength=130,shine-strength=100,rim-strength=60,glare-strength=100,shadow-strength=90,light-strength=100"),
    ("Mica", "effect=SOFT,material=GLASS,style=TINTED,opacity=92,rim=false,glare=false,shadow=false,light-follows=false,style-strength=80,shine-strength=100,rim-strength=100,glare-strength=100,shadow-strength=100,light-strength=100"),
    ("Aero", "effect=STRONG,material=GLOSS,style=TINTED,opacity=65,rim=true,glare=true,shadow=true,light-follows=false,style-strength=130,shine-strength=150,rim-strength=140,glare-strength=150,shadow-strength=100,light-strength=100"),
    ("Glassmorphism", "effect=STRONG,material=GLASS,style=CLEAR,opacity=40,rim=true,glare=false,shadow=true,light-follows=false,style-strength=100,shine-strength=100,rim-strength=120,glare-strength=100,shadow-strength=110,light-strength=100"),
    ("Vision", "effect=STRONG,material=GLASS,style=SMOKE,opacity=60,rim=true,glare=true,shadow=true,light-follows=true,style-strength=80,shine-strength=100,rim-strength=170,glare-strength=90,shadow-strength=60,light-strength=110"),
];

fn glass_section(app: &mut App, ui: &mut Ui, pal: &Palette) {
    use crate::system::GlassScope;
    use crate::theme::{GlassEffect, GlassMaterial, GlassStyle};
    let dragging = ui.input(|i| i.pointer.primary_down());
    if let Some(scope) = app.glass_dirty.filter(|_| !dragging) {
        let values = app.scope_glass(scope);
        app.set_scope_glass(scope, values, true);
    }
    widgets::section_label(ui, pal, "Glass effects");
    let scope = app.glass_edit;
    let own = app.scope_glass(scope);
    let shown = app.resolved_glass(scope);
    let mut pick_scope = None;
    let mut follow_change = None;
    let mut values = shown;
    widgets::card(ui, pal, |ui| {
        glass_row(
            ui,
            pal,
            "Apply to",
            "Global is the desktop: the same settings as Fusion Desk › Appearance",
            |ui| {
                for s in GlassScope::ALL {
                    if segment(ui, pal, s.label(), s == scope) {
                        pick_scope = Some(s);
                    }
                }
            },
        );
        if let Some(parent) = scope.parent() {
            widgets::divider(ui, pal);
            let mut follow = own.is_none();
            let title = format!("Use the {} glass", parent.label().to_lowercase());
            let sub = if parent == GlassScope::Global {
                "Look like the taskbar and menus".to_owned()
            } else {
                format!("Turn off to give {} its own glass", scope.label())
            };
            if widgets::row(
                ui,
                pal,
                pal.accent,
                Icon::Personalise,
                &title,
                &sub,
                Right::Switch(&mut follow),
            )
            .toggled
            {
                follow_change = Some(follow);
            }
        }
        if own.is_none() {
            return;
        }
        widgets::divider(ui, pal);
        glass_row(
            ui,
            pal,
            "Presets",
            "Start from a ready-made look, then fine-tune it below",
            |ui| {
                for (name, text) in GLASS_PRESETS {
                    let preset = crate::theme::Glass::from_text(text);
                    if segment(ui, pal, name, values.to_text() == preset.to_text()) {
                        values = preset;
                    }
                }
            },
        );
        widgets::divider(ui, pal);
        glass_row(
            ui,
            pal,
            "Glass effect",
            "Blur, lighter top and a bright edge",
            |ui| {
                for e in GlassEffect::ALL {
                    if segment(ui, pal, e.label(), values.effect == *e) {
                        values.effect = *e;
                    }
                }
            },
        );
        if !values.on() {
            return;
        }
        widgets::divider(ui, pal);
        glass_row(
            ui,
            pal,
            "Material",
            "Subtle shine, or strong polished shine and bevel",
            |ui| {
                for m in GlassMaterial::ALL {
                    if segment(ui, pal, m.label(), values.material == *m) {
                        values.material = *m;
                    }
                }
            },
        );
        if values.material == GlassMaterial::Gloss {
            widgets::divider(ui, pal);
            strength_row(ui, pal, "Shine strength", &mut values.shine_strength);
        }
        widgets::divider(ui, pal);
        glass_row(
            ui,
            pal,
            "Glass style",
            "Frosted is milky, Gel has a candy top, Glow lets coloured light through",
            |ui| {
                for st in GlassStyle::ALL {
                    if segment(ui, pal, st.label(), values.style == *st) {
                        values.style = *st;
                    }
                }
            },
        );
        widgets::divider(ui, pal);
        strength_row(ui, pal, "Style strength", &mut values.style_strength);
        widgets::divider(ui, pal);
        let mut t = (values.opacity as f32 - 10.0) / 90.0;
        if slider_row(
            ui,
            pal,
            Icon::Sparkle,
            "Opacity",
            format!(
                "{} % · below 100 the window shows the desktop through it",
                values.opacity
            ),
            &mut t,
        ) {
            values.opacity = ((10.0 + t * 90.0) / 5.0).round() as u8 * 5;
        }
        for (title, sub, flag, scale_title, scale) in [
            (
                "Rim highlight",
                "Thin light edge; a sharp bevel with gloss",
                &mut values.rim,
                "Rim strength",
                &mut values.rim_strength,
            ),
            (
                "Corner glare",
                "Light reflections in the corners",
                &mut values.glare,
                "Glare strength",
                &mut values.glare_strength,
            ),
            (
                "Soft shadow",
                "Diffused shadow for depth",
                &mut values.shadow,
                "Shadow strength",
                &mut values.shadow_strength,
            ),
            (
                "Light follows pointer",
                "A soft light spot moves with the pointer",
                &mut values.light_follows,
                "Light strength",
                &mut values.light_strength,
            ),
        ] {
            widgets::divider(ui, pal);
            widgets::row(
                ui,
                pal,
                pal.accent,
                Icon::Sparkle,
                title,
                sub,
                Right::Switch(flag),
            );
            if *flag {
                widgets::divider(ui, pal);
                strength_row(ui, pal, scale_title, scale);
            }
        }
    });
    let fusion = crate::desktop::fusion_installed();
    note(
        ui,
        pal,
        match (scope, fusion) {
            (GlassScope::Global, true) => "Global changes Fusion Desk's glass for the taskbar, start menu and popups at once.",
            (GlassScope::Global, false) => "Fusion Desk is not installed, so Global only sets what Harbor apps follow.",
            _ => "GNOME cannot blur what is behind an app window: with opacity below 100 the desktop shows through sharp. Fusion Desk blurs it for the taskbar and menus.",
        },
    );
    spacer(ui, 18.0);
    if let Some(s) = pick_scope {
        if s == GlassScope::Global {
            app.reload_global_glass();
        }
        app.glass_edit = s;
    } else if let Some(follow) = follow_change {
        app.set_scope_glass(scope, if follow { None } else { Some(shown) }, true);
    } else if own.is_some() && values != shown {
        app.set_scope_glass(scope, Some(values), !dragging);
    }
}

/// A 0-200 % strength slider for one glass option (100 % is normal).
fn strength_row(ui: &mut Ui, pal: &Palette, title: &str, value: &mut u8) {
    let mut t = *value as f32 / 200.0;
    let sub = format!("{} % · 100 % is normal", value);
    if slider_row(ui, pal, Icon::Sparkle, title, sub, &mut t) {
        *value = ((t * 200.0 / 5.0).round() * 5.0) as u8;
    }
}

fn glass_row(ui: &mut Ui, pal: &Palette, title: &str, sub: &str, choices: impl FnOnce(&mut Ui)) {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                egui::RichText::new(title)
                    .font(regular(14.0))
                    .color(pal.text),
            );
            ui.label(
                egui::RichText::new(sub)
                    .font(regular(12.0))
                    .color(pal.text_faint),
            );
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        let width = ui.available_width() - ROW_PAD;
        ui.allocate_ui_with_layout(
            Vec2::new(width, 30.0),
            egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
            |ui| {
                ui.set_max_width(width);
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                choices(ui);
            },
        );
    });
    ui.add_space(12.0);
}

/// One pill of a segmented choice; returns true when clicked.
fn segment(ui: &mut Ui, pal: &Palette, label: &str, selected: bool) -> bool {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(13.0), pal.text);
    let size = Vec2::new(galley.size().x + 28.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        pal.accent
    } else if resp.hovered() {
        pal.hover
    } else {
        pal.input_bg
    };
    ui.painter()
        .add(widgets::glass_fill(pal, rect, pal.corner(15), fill));
    if !selected {
        ui.painter().rect_stroke(
            rect,
            pal.corner(15),
            Stroke::new(1.0, pal.card_border),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        regular(13.0),
        if selected { pal.on_accent } else { pal.text },
    );
    resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

/// Keyboard Manager (like PowerToys): remap keys and shortcuts on every
/// keyboard with Harbor's own engine (`harbor-keymapd`).
fn keymap_section(app: &mut App, ui: &mut Ui, pal: &Palette) {
    use crate::keymap::{Combo, Keymap, Kind, Rule, MODS};
    if app.keymap_saved.is_none() {
        let saved = Keymap::load();
        app.keymap_edit = Some(saved.clone());
        app.keymap_saved = Some(saved);
    }
    if let Some(rx) = &app.keymap_rx {
        match rx.try_recv() {
            Ok(Ok(())) => {
                app.keymap_rx = None;
                app.keymap_saved = app.keymap_edit.clone();
                app.notify("Keyboard Manager rules saved");
            }
            Ok(Err(err)) => {
                app.keymap_rx = None;
                app.notify_error(&err);
            }
            Err(mpsc::TryRecvError::Empty) => {
                ui.ctx().request_repaint_after(Duration::from_millis(200))
            }
            Err(mpsc::TryRecvError::Disconnected) => app.keymap_rx = None,
        }
    }
    let saving = app.keymap_rx.is_some();
    let mut map = app.keymap_edit.clone().unwrap_or_default();
    let mut apply = false;

    // The Linux key code for a key egui reports (Caps Lock, Menu and media
    // keys never reach the app: pick those from the list).
    fn egui_code(key: egui::Key) -> Option<u16> {
        use egui::Key as K;
        let name = match key {
            K::Escape => "ESC",
            K::Tab => "TAB",
            K::Backspace => "BACKSPACE",
            K::Enter => "ENTER",
            K::Space => "SPACE",
            K::Insert => "INSERT",
            K::Delete => "DELETE",
            K::Home => "HOME",
            K::End => "END",
            K::PageUp => "PAGEUP",
            K::PageDown => "PAGEDOWN",
            K::ArrowUp => "UP",
            K::ArrowDown => "DOWN",
            K::ArrowLeft => "LEFT",
            K::ArrowRight => "RIGHT",
            K::Comma => "COMMA",
            K::Backslash => "BACKSLASH",
            K::Slash => "SLASH",
            K::OpenBracket => "LEFTBRACE",
            K::CloseBracket => "RIGHTBRACE",
            K::Backtick => "GRAVE",
            K::Minus => "MINUS",
            K::Period => "DOT",
            K::Equals => "EQUAL",
            K::Semicolon => "SEMICOLON",
            K::Quote => "APOSTROPHE",
            K::ShiftLeft => "LEFTSHIFT",
            K::ShiftRight => "RIGHTSHIFT",
            K::ControlLeft => "LEFTCTRL",
            K::ControlRight => "RIGHTCTRL",
            K::AltLeft => "LEFTALT",
            K::AltRight => "RIGHTALT",
            K::SuperLeft => "LEFTMETA",
            K::SuperRight => "RIGHTMETA",
            K::IntlBackslash => "102ND",
            other => {
                let n = other.name();
                let n = n.strip_prefix("Num").filter(|d| d.len() == 1).unwrap_or(n);
                return crate::keymap::code(&n.to_ascii_uppercase());
            }
        };
        crate::keymap::code(name)
    }

    // One key (with modifiers when `mods`): a Type button that records the
    // next key press, modifier pills and a key list. The target side also
    // has a Disable switch.
    #[allow(clippy::too_many_arguments)]
    fn combo_editor(
        ui: &mut Ui,
        pal: &Palette,
        id: (usize, u8),
        combo: &mut Combo,
        mods: bool,
        can_disable: bool,
        capture: &mut Option<(usize, u8)>,
    ) {
        let capturing = *capture == Some(id);
        if capturing {
            let (events, held_super) = ui.input(|i| {
                (
                    i.events.clone(),
                    i.keys_down.contains(&egui::Key::SuperLeft)
                        || i.keys_down.contains(&egui::Key::SuperRight),
                )
            });
            for event in events {
                let egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                else {
                    continue;
                };
                let Some(code) = egui_code(physical_key.unwrap_or(key)) else {
                    continue;
                };
                // A shortcut waits for its main key; a single key takes
                // modifiers too (Right Alt, Left Super...).
                if mods && crate::keymap::mod_bit(code) != 0 {
                    continue;
                }
                let mut bits = 0;
                if mods {
                    if modifiers.ctrl {
                        bits |= crate::keymap::MOD_CTRL;
                    }
                    if modifiers.alt {
                        bits |= crate::keymap::MOD_ALT;
                    }
                    if modifiers.shift {
                        bits |= crate::keymap::MOD_SHIFT;
                    }
                    if held_super {
                        bits |= crate::keymap::MOD_SUPER;
                    }
                }
                *combo = Combo { mods: bits, key: code };
                *capture = None;
                break;
            }
            ui.ctx().request_repaint();
        }
        let disabled = can_disable && combo.key == 0;
        if !disabled {
            let label = if *capture == Some(id) {
                "Press a key…"
            } else {
                "Type"
            };
            if segment(ui, pal, label, *capture == Some(id)) {
                *capture = if *capture == Some(id) { None } else { Some(id) };
            }
            if mods {
                for (bit, _, _) in MODS {
                    let label = match bit {
                        1 => "Ctrl",
                        2 => "Alt",
                        4 => "Shift",
                        _ => "Super",
                    };
                    if segment(ui, pal, label, combo.mods & bit != 0) {
                        combo.mods ^= bit;
                    }
                }
            }
            egui::ComboBox::from_id_salt(("keymap-key", id.0, id.1))
                .selected_text(crate::keymap::label(combo.key))
                .width(150.0)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for (_, code, label) in crate::keymap::KEYS {
                        if *code != 0 {
                            ui.selectable_value(&mut combo.key, *code, *label);
                        }
                    }
                });
        }
        if can_disable && segment(ui, pal, "Disable", disabled) {
            *combo = if disabled {
                Combo { mods: 0, key: 1 }
            } else {
                Combo { mods: 0, key: 0 }
            };
            if *capture == Some(id) {
                *capture = None;
            }
        }
    }

    widgets::section_label(ui, pal, "Keyboard Manager");
    widgets::card(ui, pal, |ui| {
        let running = system::keymap_running();
        let sub = if !map.enabled {
            "Off: every key types as usual".to_owned()
        } else if map.rules.is_empty() {
            "Add a remap below".to_owned()
        } else if running {
            format!("On: {} remap(s) on every keyboard", map.rules.len())
        } else {
            "Not running yet: select Apply".to_owned()
        };
        widgets::row(
            ui,
            pal,
            pal.accent,
            Icon::Keyboard,
            "Remap keys and shortcuts",
            &sub,
            Right::Switch(&mut map.enabled),
        );
        let mut remove = None;
        for (kind, title, sub) in [
            (
                Kind::Key,
                "Remap a key",
                "One key types another key or a shortcut, or does nothing. Select Type and press the key.",
            ),
            (
                Kind::Shortcut,
                "Remap a shortcut",
                "A shortcut types another shortcut or key",
            ),
        ] {
            widgets::divider(ui, pal);
            ui.add_space(8.0);
            ui.label(egui::RichText::new(title).size(14.5).color(pal.text));
            ui.label(egui::RichText::new(sub).size(12.0).color(pal.text_faint));
            ui.add_space(6.0);
            for (i, rule) in map.rules.iter_mut().enumerate() {
                if rule.kind != kind {
                    continue;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    combo_editor(
                        ui,
                        pal,
                        (i, 0),
                        &mut rule.from,
                        kind == Kind::Shortcut,
                        false,
                        &mut app.keymap_capture,
                    );
                    ui.label(
                        egui::RichText::new("  →  ")
                            .size(15.0)
                            .color(pal.text_muted),
                    );
                    combo_editor(ui, pal, (i, 1), &mut rule.to, true, true, &mut app.keymap_capture);
                    if ui
                        .link(egui::RichText::new("Remove").color(pal.accent))
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
                ui.add_space(4.0);
            }
            let add = if kind == Kind::Key {
                "+ Add a key remap"
            } else {
                "+ Add a shortcut remap"
            };
            if ui
                .link(egui::RichText::new(add).color(pal.accent))
                .clicked()
            {
                let from = if kind == Kind::Key {
                    Combo { mods: 0, key: 58 } // Caps Lock
                } else {
                    Combo {
                        mods: crate::keymap::MOD_CTRL | crate::keymap::MOD_ALT,
                        key: 20,
                    }
                };
                map.rules.push(Rule {
                    kind,
                    from,
                    to: Combo { mods: 0, key: 1 },
                });
            }
            ui.add_space(6.0);
        }
        if let Some(i) = remove {
            map.rules.remove(i);
            app.keymap_capture = None;
        }
        widgets::divider(ui, pal);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let changed = app.keymap_saved.as_ref() != Some(&map);
            let label = if saving { "Saving…" } else { "Apply" };
            let resp = widgets::button(ui, pal, pal.accent, label, None);
            if resp.clicked() && !saving && (changed || !running) {
                apply = true;
            }
            ui.label(
                egui::RichText::new(if changed {
                    "Unsaved changes"
                } else {
                    "Works on every keyboard, also at the login screen"
                })
                .size(12.0)
                .color(pal.text_faint),
            );
        });
        ui.add_space(6.0);
    });
    if apply {
        let text = map.text();
        let (tx, rx) = mpsc::channel();
        app.keymap_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(system::apply_keymap(&text));
        });
    }
    app.keymap_edit = Some(map);
}
