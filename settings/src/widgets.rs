//! Building blocks for both designs: cards, rows, switches, sliders, buttons.

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Id, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};

use crate::icons::{self, Icon};
use crate::theme::{regular, semibold, Palette};

pub const ROW_H: f32 = 62.0;
/// Windows 10 uses denser rows than Windows 11.
pub const ROW_H_10: f32 = 54.0;
pub const ROW_PAD: f32 = 16.0;
pub const CHIP: f32 = 28.0;

pub fn h1(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(2.0);
    let size = if pal.design == crate::theme::Design::Win10 {
        24.0
    } else {
        28.0
    };
    ui.label(
        egui::RichText::new(text)
            .font(semibold(size))
            .color(pal.text),
    );
    ui.add_space(14.0);
}

pub fn h1_back(ui: &mut Ui, pal: &Palette, text: &str) -> bool {
    // Windows 10 navigates sub-pages from the navigation pane, so its
    // page titles carry no back arrow.
    if pal.design == crate::theme::Design::Win10 {
        h1(ui, pal, text);
        return false;
    }
    let mut back = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, pal.corner(6), pal.hover);
        }
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(18.0));
        icons::paint(ui.painter(), Icon::Back, glyph, pal.text);
        if resp.clicked() {
            back = true;
        }
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        let _ = resp;
        let size = if pal.design == crate::theme::Design::Win10 {
            24.0
        } else {
            28.0
        };
        ui.label(
            egui::RichText::new(text)
                .font(semibold(size))
                .color(pal.text),
        );
    });
    ui.add_space(14.0);
    back
}

pub fn section_label(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(text)
            .font(semibold(14.0))
            .color(pal.text),
    );
    ui.add_space(6.0);
}

pub fn card<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    let frame = egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.card_border))
        .corner_radius(pal.corner(8))
        .inner_margin(Margin::ZERO);
    frame
        .show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner
}

pub fn divider(ui: &mut Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    let inset = Rect::from_min_max(
        Pos2::new(rect.left() + ROW_PAD, rect.top()),
        Pos2::new(rect.right() - ROW_PAD, rect.bottom()),
    );
    ui.painter().rect_filled(inset, 0, pal.divider);
}

/// The leading glyph of a row. Windows 11 paints it on an accent tile;
/// Windows 10 uses a flat, accent-coloured line glyph with no tile.
pub fn icon_tile(
    painter: &egui::Painter,
    rect: Rect,
    icon: Icon,
    fill: Color32,
    fg: Color32,
    pal: &Palette,
) {
    if pal.design == crate::theme::Design::Win10 {
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.78));
        icons::paint(painter, icon, glyph, pal.accent);
        return;
    }
    painter.rect_filled(rect, 6, fill);
    let glyph = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.62));
    icons::paint_square(painter, icon, glyph, fg);
}

pub enum Right<'a> {
    Chevron,
    Switch(&'a mut bool),
    AccentText(&'a str),
    Check,
    None,
}

pub struct RowOutcome {
    pub clicked: bool,
    pub toggled: bool,
}

impl RowOutcome {
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    pub fn toggled(&self) -> bool {
        self.toggled
    }
}

pub fn row(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    icon: Icon,
    title: &str,
    subtitle: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(ui, pal, accent, Some(icon), None, title, subtitle, right)
}

/// A row whose leading tile is the application's real icon when one is
/// available, with the painted vector glyph as fallback.
#[allow(dead_code)] // kept for pages that want an app row with a chevron
pub fn app_row(
    ui: &mut Ui,
    pal: &Palette,
    texture: Option<&egui::TextureHandle>,
    fallback: Icon,
    title: &str,
    subtitle: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(
        ui,
        pal,
        pal.accent,
        Some(fallback),
        texture,
        title,
        subtitle,
        right,
    )
}

pub fn info_row(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    subtitle: &str,
    right: Right<'_>,
) -> RowOutcome {
    row_impl(ui, pal, pal.accent, None, None, title, subtitle, right)
}

fn row_impl(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    icon: Option<Icon>,
    texture: Option<&egui::TextureHandle>,
    title: &str,
    subtitle: &str,
    mut right: Right<'_>,
) -> RowOutcome {
    let width = ui.available_width();
    let row_h = if pal.design == crate::theme::Design::Win10 {
        ROW_H_10
    } else {
        ROW_H
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
    let hovered = response.hovered();
    let is_switch = matches!(&right, Right::Switch(_));
    let is_plain = matches!(&right, Right::None);
    let interactive = !is_plain || icon.is_some();

    if hovered && interactive {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 4.0, rect.bottom() - 2.0),
        );
        ui.painter().rect_filled(bg, pal.corner(4), pal.hover);
    }

    let mut toggled = false;
    let text_x = if let Some(texture) = texture {
        let chip = Rect::from_center_size(
            Pos2::new(rect.left() + ROW_PAD + CHIP / 2.0, rect.center().y),
            Vec2::splat(CHIP),
        );
        egui::Image::new((texture.id(), chip.size()))
            .corner_radius(pal.corner(5))
            .paint_at(ui, chip);
        chip.right() + 14.0
    } else if let Some(icon) = icon {
        let chip = Rect::from_center_size(
            Pos2::new(rect.left() + ROW_PAD + CHIP / 2.0, rect.center().y),
            Vec2::splat(CHIP),
        );
        icon_tile(ui.painter(), chip, icon, accent, pal.on_accent, pal);
        chip.right() + 14.0
    } else {
        rect.left() + ROW_PAD + 2.0
    };

    // Keep the labels clear of whatever sits on the right of the row.
    let win10 = pal.design == crate::theme::Design::Win10;
    let reserve = match &right {
        Right::Chevron | Right::Check => 44.0,
        Right::Switch(_) => {
            if win10 {
                108.0
            } else {
                76.0
            }
        }
        Right::AccentText(text) => 30.0 + text.chars().count() as f32 * 7.2,
        Right::None => ROW_PAD,
    };
    let clip = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top()),
        Pos2::new((rect.right() - reserve).max(text_x + 40.0), rect.bottom()),
    );
    let text_painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
    let has_subtitle = !subtitle.is_empty();
    let title_y = if has_subtitle {
        rect.center().y - 8.5
    } else {
        rect.center().y
    };
    let title_font = regular(14.0);
    let title_color = pal.text;
    text_painter.text(
        Pos2::new(text_x, title_y),
        Align2::LEFT_CENTER,
        title,
        title_font,
        title_color,
    );
    if has_subtitle {
        text_painter.text(
            Pos2::new(text_x, rect.center().y + 10.5),
            Align2::LEFT_CENTER,
            subtitle,
            regular(12.0),
            pal.text_faint,
        );
    }

    let accent_text = matches!(&right, Right::AccentText(_));
    let switch_rect = match &mut right {
        Right::Chevron => {
            let chevron = Rect::from_center_size(
                Pos2::new(rect.right() - ROW_PAD - 7.0, rect.center().y),
                Vec2::splat(14.0),
            );
            icons::paint(ui.painter(), Icon::ChevronRight, chevron, pal.text_faint);
            None
        }
        Right::Switch(state) => {
            let r = Rect::from_center_size(
                Pos2::new(rect.right() - ROW_PAD - 20.0, rect.center().y),
                Vec2::new(40.0, 20.0),
            );
            let id = response.id.with("switch");
            let t = ui.ctx().animate_bool_with_time(id, **state, 0.11);
            paint_switch(ui.painter(), r, t, pal, accent);
            if win10 {
                // Windows 10 names the state next to every switch.
                ui.painter().text(
                    Pos2::new(r.left() - 12.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    if **state { "On" } else { "Off" },
                    regular(13.0),
                    pal.text_muted,
                );
            }
            Some(r)
        }
        Right::AccentText(text) => {
            let _ = accent_text;
            let color = accent;
            ui.painter().text(
                Pos2::new(rect.right() - ROW_PAD - 2.0, rect.center().y),
                Align2::RIGHT_CENTER,
                *text,
                regular(12.5),
                color,
            );
            None
        }
        Right::Check => {
            let check = Rect::from_center_size(
                Pos2::new(rect.right() - ROW_PAD - 8.0, rect.center().y),
                Vec2::splat(16.0),
            );
            icons::paint(ui.painter(), Icon::Check, check, accent);
            None
        }
        Right::None => None,
    };

    let _ = switch_rect;

    let clicked = response.clicked();
    let response = response.on_hover_cursor(if interactive {
        CursorIcon::PointingHand
    } else {
        CursorIcon::Default
    });

    if clicked {
        if let Right::Switch(state) = &mut right {
            **state = !**state;
            toggled = true;
        }
    }

    // Switch rows should not report navigation clicks.
    let clicked = clicked && !is_switch;

    let _ = response;
    let _ = hovered;
    RowOutcome { clicked, toggled }
}

pub fn paint_switch(painter: &egui::Painter, rect: Rect, t: f32, pal: &Palette, accent: Color32) {
    let win10 = pal.design == crate::theme::Design::Win10;
    if win10 {
        // Windows 10: a thin outlined track when off and a solid accent track
        // when on, with a white knob that always has a border.
        let radius = rect.height() / 2.0;
        painter.rect_filled(rect, radius, lerp_color(pal.card, accent, t));
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, lerp_color(pal.text_faint, accent, t)),
            StrokeKind::Inside,
        );
        let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
        painter.circle_filled(Pos2::new(knob_x, rect.center().y), 6.0, Color32::WHITE);
        painter.circle_stroke(
            Pos2::new(knob_x, rect.center().y),
            6.0,
            Stroke::new(1.0, pal.text_faint),
        );
        return;
    }

    let track = lerp_color(Color32::from_rgba_unmultiplied(0, 0, 0, 0), accent, t);
    painter.rect_filled(rect, rect.height() / 2.0, track);
    if t < 0.98 {
        painter.rect_stroke(
            rect,
            rect.height() / 2.0,
            Stroke::new(1.0, pal.toggle_knob_off.gamma_multiply(0.65)),
            StrokeKind::Inside,
        );
    }
    let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
    let knob_color = lerp_color(pal.toggle_knob_off, Color32::WHITE, t);
    painter.circle_filled(Pos2::new(knob_x, rect.center().y), 6.5, knob_color);
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        egui::lerp(a.r() as f32..=b.r() as f32, t) as u8,
        egui::lerp(a.g() as f32..=b.g() as f32, t) as u8,
        egui::lerp(a.b() as f32..=b.b() as f32, t) as u8,
        egui::lerp(a.a() as f32..=b.a() as f32, t) as u8,
    )
}

pub fn nav_item(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    icon: Icon,
    label: &str,
    selected: bool,
) -> Response {
    let width = ui.available_width();
    let nav_h = if pal.design == crate::theme::Design::Win10 {
        40.0
    } else {
        38.0
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, nav_h), Sense::click());

    let win10 = pal.design == crate::theme::Design::Win10;
    if win10 {
        // Windows 10: the selection fills the whole pane, with a 3 px accent
        // bar at the pane edge; hover is a flat fill too.
        if selected {
            ui.painter().rect_filled(rect, 0, pal.nav_selected);
            let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter().rect_filled(bar, 0, accent);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 0, pal.hover);
        }
        let chip = Rect::from_center_size(
            Pos2::new(rect.left() + 24.0, rect.center().y),
            Vec2::splat(22.0),
        );
        icons::paint(ui.painter(), icon, chip, pal.text);
        ui.painter().text(
            Pos2::new(rect.left() + 44.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            regular(14.0),
            pal.text,
        );
        return response.on_hover_cursor(CursorIcon::PointingHand);
    }

    if selected {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 6, pal.nav_selected);
        let pill = Rect::from_center_size(
            Pos2::new(rect.left() + 4.0, rect.center().y),
            Vec2::new(3.0, 16.0),
        );
        ui.painter().rect_filled(pill, 2, accent);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 6, pal.hover);
    }

    let chip = Rect::from_center_size(
        Pos2::new(rect.left() + 23.0, rect.center().y),
        Vec2::splat(24.0),
    );
    let tile_color = if selected {
        accent
    } else {
        accent.gamma_multiply(0.92)
    };
    icon_tile(ui.painter(), chip, icon, tile_color, pal.on_accent, pal);

    ui.painter().text(
        Pos2::new(rect.left() + 45.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        regular(14.0),
        if selected { pal.text } else { pal.text_muted },
    );

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn search_box(
    ui: &mut Ui,
    pal: &Palette,
    query: &mut String,
    focus: bool,
    hint: &str,
) -> (Response, bool) {
    let width = ui.available_width();
    let frame = egui::Frame::new()
        .fill(pal.input_bg)
        .stroke(Stroke::new(1.0, pal.input_border))
        .corner_radius(pal.corner(6))
        .inner_margin(Margin::symmetric(10, 6));
    let inner = frame.show(ui, |ui| {
        ui.set_width(width - 22.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
            icons::paint(ui.painter(), Icon::Search, icon_rect, pal.text_faint);
            let response = ui.add(
                egui::TextEdit::singleline(query)
                    .frame(egui::Frame::NONE)
                    .hint_text(
                        egui::RichText::new(hint)
                            .font(regular(13.0))
                            .color(pal.text_faint),
                    )
                    .font(regular(13.0))
                    .desired_width(ui.available_width())
                    .vertical_align(egui::Align::Center),
            );
            if focus {
                response.request_focus();
            }
            response
        })
        .inner
    });
    let response = inner.inner;
    let changed = response.changed();
    (response, changed)
}

pub fn button(
    ui: &mut Ui,
    pal: &Palette,
    accent: Color32,
    label: &str,
    icon: Option<Icon>,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(14.0), pal.on_accent);
    let extra = if icon.is_some() { 30.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 28.0 + extra, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        crate::theme::darken(accent, 18)
    } else if response.hovered() {
        crate::theme::lighten(accent, 12)
    } else {
        accent
    };
    ui.painter().rect_filled(rect, pal.corner(5), fill);
    let mut x = rect.center().x - galley.size().x / 2.0 + extra / 2.0;
    if let Some(icon) = icon {
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 16.0, rect.center().y),
            Vec2::splat(15.0),
        );
        icons::paint(ui.painter(), icon, icon_rect, pal.on_accent);
        x = icon_rect.right() + 8.0;
    }
    ui.painter().galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.on_accent,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn ghost_button(ui: &mut Ui, pal: &Palette, label: &str, icon: Option<Icon>) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), regular(14.0), pal.text);
    let extra = if icon.is_some() { 30.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 26.0 + extra, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() {
        pal.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, pal.corner(5), fill);
    ui.painter().rect_stroke(
        rect,
        5,
        Stroke::new(1.0, pal.card_border),
        StrokeKind::Inside,
    );
    let mut x = rect.center().x - galley.size().x / 2.0 + extra / 2.0;
    if let Some(icon) = icon {
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 15.0, rect.center().y),
            Vec2::splat(15.0),
        );
        icons::paint(ui.painter(), icon, icon_rect, pal.text);
        x = icon_rect.right() + 8.0;
    }
    ui.painter().galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.text,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn slider(ui: &mut Ui, pal: &Palette, accent: Color32, value: &mut f32, id: Id) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), Vec2::new(width, 4.0));
    ui.painter().rect_filled(track, 2, pal.track);

    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.clicked() || response.dragged() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            if (t - *value).abs() > f32::EPSILON {
                *value = t;
                changed = true;
            }
            ui.ctx().request_repaint();
        }
    }

    let t = value.clamp(0.0, 1.0);
    let knob = Pos2::new(rect.left() + rect.width() * t, rect.center().y);
    let filled = Rect::from_min_max(track.min, Pos2::new(knob.x, track.max.y));
    ui.painter().rect_filled(filled, pal.corner(2), accent);
    ui.painter().circle_filled(knob, 9.0, accent);
    ui.painter().circle_filled(knob, 4.0, pal.card);
    let _ = id;
    response.on_hover_cursor(CursorIcon::PointingHand);
    changed
}

pub fn avatar(ui: &mut Ui, pal: &Palette, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let dark = pal.bg.r() < 128;
    let fill = if dark {
        Color32::from_rgb(0x54, 0x54, 0x54)
    } else {
        Color32::from_rgb(0xD9, 0xD9, 0xD9)
    };
    ui.painter().circle_filled(rect.center(), size / 2.0, fill);
    let letter = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned());
    let color = if dark {
        Color32::WHITE
    } else {
        Color32::from_rgb(0x20, 0x20, 0x20)
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        letter,
        semibold(size * 0.42),
        color,
    );
}

/// Small "screen in a laptop" illustration used on the System page.
pub fn device_art(painter: &egui::Painter, rect: Rect, pal: &Palette, accent: Color32) {
    let screen = Rect::from_min_max(
        Pos2::new(rect.left() + 5.0, rect.top()),
        Pos2::new(rect.right() - 5.0, rect.bottom() - 10.0),
    );
    painter.rect_filled(screen, 4, pal.card_hover);
    painter.rect_stroke(
        screen,
        4,
        Stroke::new(1.3, pal.text_faint),
        StrokeKind::Inside,
    );
    let inside = screen.shrink(4.5);
    painter.rect_filled(inside, 2, Color32::from_rgb(0x24, 0x24, 0x24));

    // A tiny desktop: window, accent title bar and taskbar.
    let win = Rect::from_min_size(
        inside.min + Vec2::new(5.0, 7.0),
        Vec2::new(inside.width() * 0.62, inside.height() * 0.52),
    );
    painter.rect_filled(win, 1.5, Color32::from_rgb(0x39, 0x39, 0x39));
    painter.rect_filled(
        Rect::from_min_max(win.min, Pos2::new(win.right(), win.top() + 4.0)),
        1,
        accent,
    );
    painter.rect_filled(
        Rect::from_min_max(
            win.min + Vec2::new(3.0, 8.0),
            Pos2::new(win.right() - 4.0, win.top() + 9.5),
        ),
        1,
        accent.gamma_multiply(0.55),
    );
    painter.rect_filled(
        Rect::from_min_max(
            win.min + Vec2::new(3.0, 12.0),
            Pos2::new(win.right() - 10.0, win.top() + 13.5),
        ),
        1,
        pal.text_faint.gamma_multiply(0.6),
    );

    let taskbar = Rect::from_min_max(Pos2::new(inside.left(), inside.bottom() - 6.0), inside.max);
    painter.rect_filled(taskbar, 0, accent.gamma_multiply(0.9));
    painter.circle_filled(
        Pos2::new(inside.center().x, taskbar.center().y),
        1.4,
        Color32::WHITE.gamma_multiply(0.9),
    );

    let base = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.bottom() - 4.0),
        Vec2::new(rect.width() * 0.58, 3.0),
    );
    painter.rect_filled(base, 1.5, pal.text_faint);
}

pub fn caption_button(ui: &mut Ui, pal: &Palette, kind: Caption, accent: Color32) -> Response {
    // Full title-bar height: the hit area reaches the window edge.
    let height = ui.available_height().max(32.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(46.0, height), Sense::click());
    let close_hover = matches!(kind, Caption::Close) && response.hovered();
    if close_hover {
        // Square fill: the window has square corners, so a rounded corner
        // would leave a gap in the top-right corner of the screen.
        let red = if pal.design == crate::theme::Design::Win10 {
            Color32::from_rgb(0xE8, 0x11, 0x23)
        } else {
            Color32::from_rgb(0xC4, 0x2B, 0x1C)
        };
        ui.painter().rect_filled(rect, 0, red);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0, pal.hover);
    }
    let color = if close_hover {
        Color32::WHITE
    } else {
        pal.text_muted
    };
    let c = rect.center();
    let s = 5.0;
    let stroke = Stroke::new(1.0, color);
    match kind {
        Caption::Minimize => {
            ui.painter().line_segment(
                [Pos2::new(c.x - s, c.y + 0.5), Pos2::new(c.x + s, c.y + 0.5)],
                stroke,
            );
        }
        Caption::Maximize => {
            let r = Rect::from_center_size(c, Vec2::splat(9.0));
            ui.painter().rect_stroke(r, 1, stroke, StrokeKind::Inside);
        }
        Caption::Restore => {
            let r = Rect::from_center_size(c + Vec2::new(1.0, 1.0), Vec2::splat(8.0));
            ui.painter().rect_stroke(r, 1, stroke, StrokeKind::Inside);
            let r2 =
                Rect::from_min_size(Pos2::new(r.left() - 2.0, r.top() - 2.0), Vec2::splat(8.0));
            ui.painter()
                .line_segment([r2.left_top(), r2.right_top()], stroke);
            ui.painter()
                .line_segment([r2.left_top(), r2.left_bottom()], stroke);
        }
        Caption::Close => {
            ui.painter().line_segment(
                [Pos2::new(c.x - s, c.y - s), Pos2::new(c.x + s, c.y + s)],
                stroke,
            );
            ui.painter().line_segment(
                [Pos2::new(c.x - s, c.y + s), Pos2::new(c.x + s, c.y - s)],
                stroke,
            );
        }
    }
    let _ = accent;
    response
}

#[derive(Clone, Copy)]
pub enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}

pub fn empty_note(ui: &mut Ui, pal: &Palette, text: &str) {
    let width = ui.available_width();
    ui.add_space(18.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, 24.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(text)
                    .font(regular(13.0))
                    .color(pal.text_faint),
            );
        },
    );
    ui.add_space(18.0);
}
