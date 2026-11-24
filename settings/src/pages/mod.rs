//! All pages. Data comes from `system::Info`; actions are applied immediately
//! through gsettings/pactl/nmcli/bluetoothctl and reported with a toast.

mod about;
mod accessibility;
mod accounts;
mod apps;
mod bluetooth;
mod compatibility;
mod default_apps;
mod display;
mod home;
mod keyboard;
mod network;
mod notifications;
mod personalisation;
mod pointer;
mod power;
mod privacy;
mod search;
mod sound;
mod storage;
mod system_page;
mod time_language;
mod updates;

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use crate::app::App;
use crate::icons::Icon;
use crate::theme::{regular, semibold, with_alpha, Palette};
use crate::widgets::{self, ROW_PAD};

pub use about::*;
pub use accessibility::*;
pub use accounts::*;
pub use apps::*;
pub use bluetooth::*;
pub use compatibility::*;
pub use default_apps::*;
pub use display::*;
pub use home::*;
pub use keyboard::*;
pub use network::*;
pub use notifications::*;
pub use personalisation::*;
pub use pointer::*;
pub use power::*;
pub use privacy::*;
pub use search::*;
pub use sound::*;
pub use storage::*;
pub use system_page::*;
pub use time_language::*;
pub use updates::*;

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
