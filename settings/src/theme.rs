//! Fluent / Windows 11 inspired colours, fonts and spacing.

use eframe::egui::{
    self, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use std::sync::Arc;

pub const FAMILY_SEMIBOLD: &str = "selawik-semibold";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Dark,
    Light,
}

/// Accent swatches offered on the Personalisation page.
pub const ACCENTS: [(&str, Color32); 8] = [
    ("Rose", Color32::from_rgb(0xD6, 0x33, 0x6B)),
    ("Blue", Color32::from_rgb(0x00, 0x78, 0xD4)),
    ("Teal", Color32::from_rgb(0x03, 0x83, 0x87)),
    ("Purple", Color32::from_rgb(0x87, 0x64, 0xB8)),
    ("Orange", Color32::from_rgb(0xCA, 0x50, 0x10)),
    ("Green", Color32::from_rgb(0x10, 0x7C, 0x10)),
    ("Gold", Color32::from_rgb(0x98, 0x6F, 0x0B)),
    ("Magenta", Color32::from_rgb(0xC2, 0x39, 0xB3)),
];

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub card_border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub divider: Color32,
    pub input_bg: Color32,
    pub input_border: Color32,
    pub toggle_knob_off: Color32,
    pub track: Color32,
    pub nav_selected: Color32,
    pub tooltip: Color32,
}

impl Palette {
    pub fn new(mode: Mode, accent: Color32) -> Self {
        match mode {
            Mode::Dark => Self {
                bg: Color32::from_rgb(0x20, 0x20, 0x20),
                card: Color32::from_rgb(0x2B, 0x2B, 0x2B),
                card_hover: Color32::from_rgb(0x32, 0x32, 0x32),
                card_border: Color32::from_rgb(0x38, 0x38, 0x38),
                text: Color32::from_rgb(0xF5, 0xF5, 0xF5),
                text_muted: Color32::from_rgb(0xCF, 0xCF, 0xCF),
                text_faint: Color32::from_rgb(0x93, 0x93, 0x93),
                accent,
                on_accent: Color32::WHITE,
                hover: Color32::from_rgb(0x33, 0x33, 0x33),
                pressed: Color32::from_rgb(0x2A, 0x2A, 0x2A),
                divider: Color32::from_rgb(0x3A, 0x3A, 0x3A),
                input_bg: Color32::from_rgb(0x2D, 0x2D, 0x2D),
                input_border: Color32::from_rgb(0x3D, 0x3D, 0x3D),
                toggle_knob_off: Color32::from_rgb(0xCF, 0xCF, 0xCF),
                track: Color32::from_rgb(0x4A, 0x4A, 0x4A),
                nav_selected: Color32::from_rgb(0x2E, 0x2E, 0x2E),
                tooltip: Color32::from_rgb(0x2D, 0x2D, 0x2D),
            },
            Mode::Light => Self {
                bg: Color32::from_rgb(0xF3, 0xF3, 0xF3),
                card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                card_hover: Color32::from_rgb(0xFA, 0xFA, 0xFA),
                card_border: Color32::from_rgb(0xE3, 0xE3, 0xE3),
                text: Color32::from_rgb(0x1B, 0x1B, 0x1B),
                text_muted: Color32::from_rgb(0x5A, 0x5A, 0x5A),
                text_faint: Color32::from_rgb(0x77, 0x77, 0x77),
                accent,
                on_accent: Color32::WHITE,
                hover: Color32::from_rgb(0xEC, 0xEC, 0xEC),
                pressed: Color32::from_rgb(0xE2, 0xE2, 0xE2),
                divider: Color32::from_rgb(0xE5, 0xE5, 0xE5),
                input_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                input_border: Color32::from_rgb(0xD6, 0xD6, 0xD6),
                toggle_knob_off: Color32::from_rgb(0x5F, 0x5F, 0x5F),
                track: Color32::from_rgb(0xC9, 0xC9, 0xC9),
                nav_selected: Color32::from_rgb(0xE7, 0xE7, 0xE7),
                tooltip: Color32::from_rgb(0xF0, 0xF0, 0xF0),
            },
        }
    }
}

pub fn lighten(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(amount),
        c.g().saturating_add(amount),
        c.b().saturating_add(amount),
    )
}

pub fn darken(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_sub(amount),
        c.g().saturating_sub(amount),
        c.b().saturating_sub(amount),
    )
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_SEMIBOLD.into()))
}

/// Register Selawik (Segoe UI compatible) and a small fluent type ramp.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "selawik".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/selawk.ttf"
        ))),
    );
    fonts.font_data.insert(
        "selawik-semibold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/selawksb.ttf"
        ))),
    );
    fonts.font_data.insert(
        "selawik-light".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/selawkl.ttf"
        ))),
    );

    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "selawik".to_owned());
    }
    if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
        list.push("selawik".to_owned());
    }
    fonts.families.insert(
        FontFamily::Name(FAMILY_SEMIBOLD.into()),
        vec!["selawik-semibold".to_owned(), "selawik".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("selawik-light".into()),
        vec!["selawik-light".to_owned(), "selawik".to_owned()],
    );

    ctx.set_fonts(fonts);
}

pub fn apply_style(ctx: &Context, pal: &Palette, mode: Mode) {
    let theme = match mode {
        Mode::Dark => egui::Theme::Dark,
        Mode::Light => egui::Theme::Light,
    };
    ctx.set_theme(theme);
    let mut style = (*ctx.style_of(theme)).clone();

    style.visuals = match mode {
        Mode::Dark => egui::Visuals::dark(),
        Mode::Light => egui::Visuals::light(),
    };
    style.visuals.panel_fill = pal.bg;
    style.visuals.window_fill = pal.card;
    style.visuals.extreme_bg_color = pal.input_bg;
    style.visuals.faint_bg_color = pal.card;
    style.visuals.override_text_color = Some(pal.text);
    style.visuals.selection.bg_fill = with_alpha(pal.accent, 140);
    style.visuals.selection.stroke = Stroke::new(1.0, pal.text);
    style.visuals.hyperlink_color = pal.accent;
    style.visuals.window_stroke = Stroke::new(1.0, pal.card_border);
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, pal.text_muted);
    style.visuals.widgets.inactive.weak_bg_fill = pal.card;
    style.visuals.widgets.inactive.bg_fill = pal.card;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, pal.text);
    style.visuals.widgets.hovered.weak_bg_fill = pal.card_hover;
    style.visuals.widgets.hovered.bg_fill = pal.card_hover;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, pal.text);
    style.visuals.widgets.active.weak_bg_fill = pal.pressed;
    style.visuals.widgets.active.bg_fill = pal.pressed;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, pal.text);

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size = egui::vec2(32.0, 32.0);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;

    style.text_styles = [
        (TextStyle::Heading, semibold(28.0)),
        (TextStyle::Body, regular(14.0)),
        (TextStyle::Button, regular(14.0)),
        (TextStyle::Small, regular(12.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();

    ctx.set_style_of(theme, style);
}
