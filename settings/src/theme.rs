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

macro_rules! glass_choice {
    ($name:ident { $($var:ident = $key:literal, $label:literal;)* }) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $name {
            $($var),*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),*];

            /// The value Fusion Desk stores (`STRONG`, `GLOSS`, `FROSTED`, ...).
            pub fn key(self) -> &'static str {
                match self {
                    $($name::$var => $key),*
                }
            }

            pub fn label(self) -> &'static str {
                match self {
                    $($name::$var => $label),*
                }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                let key = key.trim().trim_matches('\'');
                Self::ALL.iter().copied().find(|v| v.key().eq_ignore_ascii_case(key))
            }
        }
    };
}

glass_choice!(GlassEffect {
    Off = "OFF", "Off";
    Soft = "SOFT", "Soft";
    Strong = "STRONG", "Strong";
});

glass_choice!(GlassMaterial {
    Glass = "GLASS", "Clear glass";
    Gloss = "GLOSS", "Gloss";
});

glass_choice!(GlassStyle {
    Clear = "CLEAR", "Clear";
    Frosted = "FROSTED", "Frosted";
    Gel = "GEL", "Gel";
    Tinted = "TINTED", "Tinted";
    Smoke = "SMOKE", "Smoke";
    Halo = "HALO", "Halo";
    Glow = "GLOW", "Glow";
    Planner = "PLANNER", "Planner";
});

/// Glass styling shared by every Harbor app and Fusion Desk. The fields are
/// Fusion Desk's `glass-*` settings, so one set of values can drive the
/// desktop shell and every app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Glass {
    pub effect: GlassEffect,
    pub material: GlassMaterial,
    pub style: GlassStyle,
    /// Percent; below 100 the window is see-through.
    pub opacity: u8,
    pub rim: bool,
    pub glare: bool,
    pub shadow: bool,
    pub light_follows: bool,
    /// Strength scales in percent (0-200, 100 = normal): style shading,
    /// gloss shine, rim, glare, shadow and pointer light.
    pub style_strength: u8,
    pub shine_strength: u8,
    pub rim_strength: u8,
    pub glare_strength: u8,
    pub shadow_strength: u8,
    pub light_strength: u8,
}

impl Default for Glass {
    /// Fusion Desk's defaults.
    fn default() -> Self {
        Glass {
            effect: GlassEffect::Strong,
            material: GlassMaterial::Glass,
            style: GlassStyle::Frosted,
            opacity: 75,
            rim: true,
            glare: true,
            shadow: false,
            light_follows: false,
            style_strength: 100,
            shine_strength: 100,
            rim_strength: 100,
            glare_strength: 100,
            shadow_strength: 100,
            light_strength: 100,
        }
    }
}

impl Glass {
    pub const OFF: Glass = Glass {
        effect: GlassEffect::Off,
        material: GlassMaterial::Glass,
        style: GlassStyle::Frosted,
        opacity: 100,
        rim: true,
        glare: true,
        shadow: false,
        light_follows: false,
        style_strength: 100,
        shine_strength: 100,
        rim_strength: 100,
        glare_strength: 100,
        shadow_strength: 100,
        light_strength: 100,
    };

    /// Fusion Desk key names, in the order they are written.
    pub const KEYS: [&'static str; 14] = [
        "glass-effect",
        "glass-material",
        "glass-style",
        "glass-opacity",
        "glass-rim",
        "glass-glare",
        "glass-shadow",
        "glass-light-follows",
        "glass-style-strength",
        "glass-shine-strength",
        "glass-rim-strength",
        "glass-glare-strength",
        "glass-shadow-strength",
        "glass-light-strength",
    ];

    /// A strength as a scale (1.0 = normal).
    pub fn scale(percent: u8) -> f32 {
        percent as f32 / 100.0
    }

    pub fn on(&self) -> bool {
        self.effect != GlassEffect::Off
    }

    /// The window shows the desktop through it.
    pub fn see_through(&self) -> bool {
        self.on() && self.opacity < 100
    }

    /// One setting as Fusion Desk stores it (GVariant text: `'STRONG'`, `75`, `true`).
    pub fn get(&self, key: &str) -> String {
        let quote = |v: &str| format!("'{v}'");
        match key {
            "glass-effect" => quote(self.effect.key()),
            "glass-material" => quote(self.material.key()),
            "glass-style" => quote(self.style.key()),
            "glass-opacity" => self.opacity.to_string(),
            "glass-rim" => self.rim.to_string(),
            "glass-glare" => self.glare.to_string(),
            "glass-shadow" => self.shadow.to_string(),
            "glass-light-follows" => self.light_follows.to_string(),
            "glass-style-strength" => self.style_strength.to_string(),
            "glass-shine-strength" => self.shine_strength.to_string(),
            "glass-rim-strength" => self.rim_strength.to_string(),
            "glass-glare-strength" => self.glare_strength.to_string(),
            "glass-shadow-strength" => self.shadow_strength.to_string(),
            "glass-light-strength" => self.light_strength.to_string(),
            _ => String::new(),
        }
    }

    /// Set one setting from Fusion Desk text; unknown keys and values are ignored.
    pub fn set(&mut self, key: &str, value: &str) {
        let value = value.trim();
        let flag = |v: &str| v.trim_matches('\'') == "true";
        match key.trim().trim_start_matches("glass-") {
            "effect" => {
                if let Some(v) = GlassEffect::from_key(value) {
                    self.effect = v;
                }
            }
            "material" => {
                if let Some(v) = GlassMaterial::from_key(value) {
                    self.material = v;
                }
            }
            "style" => {
                if let Some(v) = GlassStyle::from_key(value) {
                    self.style = v;
                }
            }
            "opacity" => {
                if let Ok(v) = value.trim_matches('\'').parse::<u8>() {
                    self.opacity = v.clamp(10, 100);
                }
            }
            "rim" => self.rim = flag(value),
            "glare" => self.glare = flag(value),
            "shadow" => self.shadow = flag(value),
            "light-follows" => self.light_follows = flag(value),
            name if name.ends_with("-strength") => {
                let Ok(v) = value.trim_matches('\'').parse::<u16>() else {
                    return;
                };
                let v = v.min(200) as u8;
                match name {
                    "style-strength" => self.style_strength = v,
                    "shine-strength" => self.shine_strength = v,
                    "rim-strength" => self.rim_strength = v,
                    "glare-strength" => self.glare_strength = v,
                    "shadow-strength" => self.shadow_strength = v,
                    "light-strength" => self.light_strength = v,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// `effect=STRONG` lines, the format of Harbor's glass files.
    pub fn to_text(&self) -> String {
        Self::KEYS
            .iter()
            .map(|k| {
                format!(
                    "{}={}\n",
                    k.trim_start_matches("glass-"),
                    self.get(k).trim_matches('\'')
                )
            })
            .collect()
    }

    /// Parse `key=value` pairs (lines or commas). The old one-word styles
    /// (`off`, `glossy`, `liquid`) still work.
    pub fn from_text(text: &str) -> Glass {
        match text.trim() {
            "off" => return Glass::OFF,
            "glossy" => {
                return Glass {
                    effect: GlassEffect::Soft,
                    material: GlassMaterial::Gloss,
                    style: GlassStyle::Gel,
                    opacity: 100,
                    ..Glass::default()
                }
            }
            "liquid" => {
                return Glass {
                    style: GlassStyle::Clear,
                    opacity: 80,
                    ..Glass::default()
                }
            }
            _ => {}
        }
        let mut glass = Glass::default();
        for pair in text.split([',', '\n']) {
            if let Some((k, v)) = pair.split_once('=') {
                glass.set(k, v);
            }
        }
        glass
    }
}

/// How icons and switches are coloured (Personalisation › Colour style).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Look {
    /// Everything in the accent colour.
    #[default]
    Accent,
    /// Each icon (and its row's switch) gets its own colour from `COLOURFUL`.
    Colourful,
}

impl Look {
    pub const ALL: [Look; 2] = [Look::Accent, Look::Colourful];

    pub fn label(self) -> &'static str {
        match self {
            Look::Accent => "Accent",
            Look::Colourful => "Colourful",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Look::Accent => "accent",
            Look::Colourful => "colourful",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "accent" => Some(Look::Accent),
            "colourful" | "colorful" => Some(Look::Colourful),
            _ => None,
        }
    }
}

/// Colours the Colourful look picks from; all carry a white glyph and read
/// on light and dark cards.
pub const COLOURFUL: [Color32; 8] = [
    Color32::from_rgb(0x00, 0x78, 0xD4), // blue
    Color32::from_rgb(0x10, 0x7C, 0x10), // green
    Color32::from_rgb(0xCA, 0x50, 0x10), // orange
    Color32::from_rgb(0x87, 0x64, 0xB8), // purple
    Color32::from_rgb(0x03, 0x83, 0x87), // teal
    Color32::from_rgb(0xD6, 0x33, 0x6B), // rose
    Color32::from_rgb(0x98, 0x6F, 0x0B), // gold
    Color32::from_rgb(0xC2, 0x39, 0xB3), // magenta
];

/// Which Windows design the app paints. The settings, pages and behaviour are
/// identical; only the chrome, palette and home screen change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Design {
    Win11,
    Win10,
}

impl Design {
    pub fn label(self) -> &'static str {
        match self {
            Design::Win11 => "Windows 11",
            Design::Win10 => "Windows 10",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Design::Win11 => "win11",
            Design::Win10 => "win10",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "win11" | "11" | "windows11" => Some(Design::Win11),
            "win10" | "10" | "windows10" => Some(Design::Win10),
            _ => None,
        }
    }
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
    /// The navigation pane / Windows 10 home header band.
    pub nav_bg: Color32,
    pub tooltip: Color32,
    /// Which design this palette belongs to; widgets use it for corners,
    /// icon tiles and navigation styling.
    pub design: Design,
    /// Glass styling (Personalisation › Glass effects).
    pub glass: Glass,
    /// Accent or Colourful icons (see `tint`).
    pub look: Look,
    /// The pointer, for "Light follows pointer".
    pub pointer: Option<egui::Pos2>,
}

impl Palette {
    pub fn new(mode: Mode, accent: Color32, design: Design) -> Self {
        if design == Design::Win10 {
            return Self::win10(mode, accent);
        }
        Self::win11(mode, accent)
    }

    /// Apply a glass style: cards, inputs and panes take the style's tint and
    /// the opacity, so the glow backdrop (and, in a see-through window, the
    /// desktop) shows through them.
    /// The colour for an icon tile or a row's switch. Widgets ask with the
    /// accent; in the Colourful look the accent becomes a colour picked by
    /// `seed` (a row title or icon name), so the same item keeps its colour.
    /// Any other colour a page chose on purpose is kept.
    pub fn tint(&self, color: Color32, seed: &str) -> Color32 {
        if self.look != Look::Colourful || color != self.accent {
            return color;
        }
        // FNV-1a: stable between runs, unlike the std hasher.
        let hash = seed.bytes().fold(0x811c_9dc5u32, |h, b| {
            (h ^ b as u32).wrapping_mul(0x0100_0193)
        });
        COLOURFUL[hash as usize % COLOURFUL.len()]
    }

    pub fn with_glass(mut self, glass: Glass, see_through: bool) -> Self {
        self.glass = glass;
        if !glass.on() {
            return self;
        }
        let dark = self.bg.r() < 128;
        if glass.style == GlassStyle::Planner {
            return self.planner(glass, dark, see_through);
        }
        let accent = self.accent;
        let mix = |a: Color32, b: Color32, t: f32| {
            let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
        };
        let tint = |c: Color32| match glass.style {
            GlassStyle::Frosted => mix(c, Color32::WHITE, if dark { 0.06 } else { 0.30 }),
            GlassStyle::Tinted => mix(c, accent, if dark { 0.16 } else { 0.12 }),
            GlassStyle::Smoke => mix(c, Color32::BLACK, if dark { 0.30 } else { 0.10 }),
            GlassStyle::Glow if dark => mix(c, Color32::from_rgb(44, 46, 50), 0.6),
            _ => c,
        };
        let strong = glass.effect == GlassEffect::Strong;
        // Card opacity follows the setting; Clear is lighter, Soft less so.
        let mut alpha = glass.opacity as f32 / 100.0;
        if glass.style == GlassStyle::Clear {
            alpha *= 0.7;
        }
        if !strong {
            alpha = 0.5 + alpha * 0.5;
        }
        if glass.style == GlassStyle::Glow {
            // The coloured light behind must show through.
            alpha = alpha.min(0.6);
        }
        let see = |c: Color32, a: f32| {
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.2, 1.0) * 255.0) as u8)
        };
        self.card = see(tint(self.card), alpha);
        self.card_hover = see(tint(self.card_hover), alpha + 0.12);
        self.input_bg = see(tint(self.input_bg), alpha + 0.1);
        if glass.rim {
            self.card_border = if dark {
                Color32::from_white_alpha(if strong { 34 } else { 24 })
            } else {
                Color32::from_white_alpha(if strong { 170 } else { 120 })
            };
        }
        if see_through {
            let page = (glass.opacity as f32 / 100.0).max(0.35);
            self.bg = see(self.bg, page);
            self.nav_bg = see(self.nav_bg, page * 0.8);
        }
        self
    }

    /// The Daily Routine Planner look (spec: ~/Music/apps/glass-theme/PLANNER-SPEC.md):
    /// indigo accent, white/80 (dark zinc/80) cards with a white rim over the
    /// lavender/sky (dark navy) gradient that `widgets::glow_backdrop` paints.
    /// PLANNER palette v2 ("Planner Mist"): one violet-indigo hue family.
    /// Mist backdrop (dark: ink-violet), solid cards (slightly see-through
    /// only while Fusion Desk blurs behind the window), indigo-tinted ink
    /// text, hovers and inputs. Spec: ~/Music/apps/glass-theme/PLANNER-SPEC.md
    fn planner(mut self, glass: Glass, dark: bool, see_through: bool) -> Self {
        let rgba = |r, g, b, a: f32| Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0) as u8);
        let rgb = Color32::from_rgb;
        self.accent = rgb(0x63, 0x66, 0xf1);
        self.on_accent = Color32::WHITE;
        if dark {
            let card_a = if see_through { 0.86 } else { 1.0 };
            self.card = rgba(0x1f, 0x1c, 0x33, card_a);
            self.card_hover = rgba(0x2a, 0x26, 0x44, card_a);
            self.card_border = rgba(0xc4, 0xb5, 0xfd, 0.12);
            self.text = rgb(0xec, 0xeb, 0xf7);
            self.text_muted = rgb(0xa9, 0xa6, 0xc4);
            self.text_faint = rgb(0x6f, 0x6c, 0x8c);
            self.hover = rgba(0x8b, 0x80, 0xf9, 0.16);
            self.pressed = rgba(0x8b, 0x80, 0xf9, 0.26);
            self.divider = rgb(0x2c, 0x29, 0x45);
            self.input_bg = rgb(0x29, 0x26, 0x3f);
            self.input_border = rgba(0xc4, 0xb5, 0xfd, 0.18);
            self.track = rgb(0x33, 0x2f, 0x52);
            self.nav_selected = rgba(0x81, 0x8c, 0xf8, 0.22);
            // Opaque on purpose: code tells dark from light by bg.r(), and
            // egui stores colours premultiplied. The backdrop paints over it.
            self.bg = rgb(0x17, 0x14, 0x30);
            self.nav_bg = rgba(0x13, 0x11, 0x1f, 0.45);
            self.tooltip = rgb(0x24, 0x21, 0x3a);
        } else {
            let card_a = if see_through { 0.90 } else { 1.0 };
            self.card = rgba(0xff, 0xff, 0xff, card_a);
            self.card_hover = rgb(0xf3, 0xf1, 0xff);
            self.card_border = rgb(0xe6, 0xe3, 0xf6);
            self.text = rgb(0x1e, 0x1b, 0x3a);
            self.text_muted = rgb(0x5f, 0x5b, 0x7a);
            self.text_faint = rgb(0x9a, 0x97, 0xb3);
            self.hover = rgb(0xe9, 0xe6, 0xff);
            self.pressed = rgb(0xde, 0xda, 0xff);
            self.divider = rgb(0xec, 0xe9, 0xf8);
            self.input_bg = rgb(0xf7, 0xf6, 0xfd);
            self.input_border = rgb(0xdc, 0xd8, 0xf0);
            self.track = rgb(0xe4, 0xe0, 0xff);
            self.nav_selected = rgba(0x63, 0x66, 0xf1, 0.12);
            self.bg = rgb(0xee, 0xf0, 0xff);
            self.nav_bg = rgba(0xff, 0xff, 0xff, 0.45);
            self.tooltip = Color32::WHITE;
        }
        let _ = glass; // rim/effect only change the other styles
        self
    }

    /// Corner radius for this design: Windows 11 rounds, Windows 10 is square.
    pub fn corner(&self, radius: u8) -> u8 {
        if self.design == Design::Win10 {
            // Windows 10 with a modern touch: small, soft corners (max 4 px).
            radius.min(4)
        } else if self.glass.on() && self.glass.style == GlassStyle::Planner {
            // Planner corner scale: 8 -> 16 px cards, 6 -> 12 px controls,
            // larger panels to 24 px.
            match radius {
                0 => 0,
                1..=4 => 8,
                5..=6 => 12,
                7..=10 => 16,
                _ => 24,
            }
        } else {
            radius
        }
    }

    fn win10(mode: Mode, accent: Color32) -> Self {
        let design = Design::Win10;
        // GNOME's default blue is Adwaita's #3584E4; Windows 10's blue is
        // #0078D7, so the design keeps its own accent when the desktop uses
        // the stock GNOME blue.
        let accent = if (accent.r(), accent.g(), accent.b()) == (0x35, 0x84, 0xE4) {
            Color32::from_rgb(0x00, 0x78, 0xD7)
        } else {
            accent
        };
        match mode {
            Mode::Dark => Self {
                bg: Color32::from_rgb(0x1A, 0x1A, 0x1A),
                card: Color32::from_rgb(0x24, 0x24, 0x24),
                card_hover: Color32::from_rgb(0x2D, 0x2D, 0x2D),
                card_border: Color32::from_rgb(0x2E, 0x2E, 0x2E),
                text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                text_muted: Color32::from_rgb(0xD0, 0xD0, 0xD0),
                text_faint: Color32::from_rgb(0x9A, 0x9A, 0x9A),
                accent,
                on_accent: Color32::WHITE,
                hover: Color32::from_rgb(0x2D, 0x2D, 0x2D),
                pressed: Color32::from_rgb(0x38, 0x38, 0x38),
                divider: Color32::from_rgb(0x30, 0x30, 0x30),
                input_bg: Color32::from_rgb(0x24, 0x24, 0x24),
                input_border: Color32::from_rgb(0x5A, 0x5A, 0x5A),
                toggle_knob_off: Color32::from_rgb(0x9A, 0x9A, 0x9A),
                track: Color32::from_rgb(0x5A, 0x5A, 0x5A),
                nav_selected: Color32::from_rgb(0x33, 0x33, 0x33),
                nav_bg: Color32::from_rgb(0x20, 0x20, 0x20),
                tooltip: Color32::from_rgb(0x2B, 0x2B, 0x2B),
                design,
                glass: Glass::OFF,
                look: Look::Accent,
                pointer: None,
            },
            Mode::Light => Self {
                bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                card: Color32::from_rgb(0xF7, 0xF7, 0xF7),
                card_hover: Color32::from_rgb(0xEF, 0xEF, 0xEF),
                card_border: Color32::from_rgb(0xEB, 0xEB, 0xEB),
                text: Color32::from_rgb(0x00, 0x00, 0x00),
                text_muted: Color32::from_rgb(0x4D, 0x4D, 0x4D),
                text_faint: Color32::from_rgb(0x6E, 0x6E, 0x6E),
                accent,
                on_accent: Color32::WHITE,
                hover: Color32::from_rgb(0xEA, 0xEA, 0xEA),
                pressed: Color32::from_rgb(0xDC, 0xDC, 0xDC),
                divider: Color32::from_rgb(0xE6, 0xE6, 0xE6),
                input_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                input_border: Color32::from_rgb(0x8A, 0x8A, 0x8A),
                toggle_knob_off: Color32::from_rgb(0x76, 0x76, 0x76),
                track: Color32::from_rgb(0xCC, 0xCC, 0xCC),
                nav_selected: Color32::from_rgb(0xE3, 0xE3, 0xE3),
                nav_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3),
                tooltip: Color32::from_rgb(0xF2, 0xF2, 0xF2),
                design,
                glass: Glass::OFF,
                look: Look::Accent,
                pointer: None,
            },
        }
    }

    fn win11(mode: Mode, accent: Color32) -> Self {
        let design = Design::Win11;
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
                nav_bg: Color32::from_rgb(0x20, 0x20, 0x20),
                tooltip: Color32::from_rgb(0x2D, 0x2D, 0x2D),
                design,
                glass: Glass::OFF,
                look: Look::Accent,
                pointer: None,
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
                nav_bg: Color32::from_rgb(0xF3, 0xF3, 0xF3),
                tooltip: Color32::from_rgb(0xF0, 0xF0, 0xF0),
                design,
                glass: Glass::OFF,
                look: Look::Accent,
                pointer: None,
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

    // Fallback for text Selawik lacks (Cyrillic, Greek, symbols in device or
    // network names): the system's DejaVu, read at start instead of bundling
    // egui's default fonts into the binary.
    let mut fallback = Vec::new();
    for (name, path) in [
        (
            "sys-sans",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ),
        (
            "sys-mono",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        ),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
            fallback.push(name);
        }
    }
    let with_fallback = |first: &[&str]| -> Vec<String> {
        first
            .iter()
            .chain(fallback.iter().filter(|n| **n == "sys-sans"))
            .map(|n| (*n).to_owned())
            .collect()
    };
    fonts
        .families
        .insert(FontFamily::Proportional, with_fallback(&["selawik"]));
    let mut mono: Vec<String> = fallback
        .iter()
        .filter(|n| **n == "sys-mono")
        .map(|n| (*n).to_owned())
        .collect();
    mono.extend(with_fallback(&["selawik"]));
    fonts.families.insert(FontFamily::Monospace, mono);
    fonts.families.insert(
        FontFamily::Name(FAMILY_SEMIBOLD.into()),
        with_fallback(&["selawik-semibold", "selawik"]),
    );
    fonts.families.insert(
        FontFamily::Name("selawik-light".into()),
        with_fallback(&["selawik-light", "selawik"]),
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
    style.visuals.window_corner_radius = egui::CornerRadius::same(pal.corner(8));
    style.visuals.menu_corner_radius = egui::CornerRadius::same(pal.corner(6));
    let widget_radius = egui::CornerRadius::same(pal.corner(6));
    style.visuals.widgets.noninteractive.corner_radius = widget_radius;
    style.visuals.widgets.inactive.corner_radius = widget_radius;
    style.visuals.widgets.hovered.corner_radius = widget_radius;
    style.visuals.widgets.active.corner_radius = widget_radius;
    style.visuals.widgets.open.corner_radius = widget_radius;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_text_round_trip() {
        let g = Glass::from_text("effect=SOFT,material=GLOSS,style=GLOW,opacity=60,rim=false");
        assert_eq!(g.effect, GlassEffect::Soft);
        assert_eq!(g.style, GlassStyle::Glow);
        assert_eq!(g.opacity, 60);
        assert!(!g.rim && g.glare);
        assert_eq!(Glass::from_text(&g.to_text()), g);
        assert_eq!(g.get("glass-style"), "'GLOW'");
    }

    #[test]
    fn glass_old_words_and_bad_values() {
        assert!(!Glass::from_text("off").on());
        assert_eq!(Glass::from_text("glossy").material, GlassMaterial::Gloss);
        assert!(Glass::from_text("liquid").see_through());
        let g = Glass::from_text("style=NEON,opacity=5,effect=huge");
        assert_eq!(g.style, GlassStyle::Frosted);
        assert_eq!(g.opacity, 10);
        assert_eq!(g.effect, GlassEffect::Strong);
    }
}
