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
    Planner = "PLANNER", "Lumina Glass";
});

glass_choice!(LuminaStyle {
    Calm = "CALM", "Calm glass";
    Colourful = "COLOURFUL", "Colourful";
});

glass_choice!(LuminaColours {
    Aurora = "AURORA", "Aurora";
    Sunset = "SUNSET", "Sunset";
    Ocean = "OCEAN", "Ocean";
    Candy = "CANDY", "Candy";
    Forest = "FOREST", "Forest";
    Accent = "ACCENT", "System accent";
    Custom = "CUSTOM", "Custom";
});

impl LuminaColours {
    /// The three colours of a fixed set (same values as planner_glass.py).
    pub fn set(self) -> Option<[[u8; 3]; 3]> {
        Some(match self {
            LuminaColours::Aurora => [[0xec, 0x48, 0x99], [0x8b, 0x5c, 0xf6], [0x38, 0xbd, 0xf8]],
            LuminaColours::Sunset => [[0xf9, 0x73, 0x16], [0xec, 0x48, 0x99], [0x8b, 0x5c, 0xf6]],
            LuminaColours::Ocean => [[0x06, 0xb6, 0xd4], [0x3b, 0x82, 0xf6], [0x63, 0x66, 0xf1]],
            LuminaColours::Candy => [[0xf4, 0x72, 0xb6], [0xa7, 0x8b, 0xfa], [0x34, 0xd3, 0x99]],
            LuminaColours::Forest => [[0x22, 0xc5, 0x5e], [0x14, 0xb8, 0xa6], [0x0e, 0xa5, 0xe9]],
            LuminaColours::Accent | LuminaColours::Custom => return None,
        })
    }
}

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
    /// Lumina (style PLANNER) options: Calm (the indigo look) or Colourful
    /// (a colour set, the system accent or a custom colour, with an intensity).
    pub lumina: LuminaStyle,
    pub lumina_colours: LuminaColours,
    pub lumina_colour: [u8; 3],
    /// Percent, 20-100.
    pub lumina_intensity: u8,
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
            lumina: LuminaStyle::Calm,
            lumina_colours: LuminaColours::Aurora,
            lumina_colour: [0x8b, 0x5c, 0xf6],
            lumina_intensity: 70,
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
        lumina: LuminaStyle::Calm,
        lumina_colours: LuminaColours::Aurora,
        lumina_colour: [0x8b, 0x5c, 0xf6],
        lumina_intensity: 70,
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

    /// The Lumina keys. Written after KEYS and best effort: an older Fusion
    /// Desk has no such settings.
    pub const LUMINA_KEYS: [&'static str; 4] = [
        "glass-lumina",
        "glass-lumina-colours",
        "glass-lumina-colour",
        "glass-lumina-intensity",
    ];

    /// Lumina's three colours for Colourful (None = Calm). `accent` is the
    /// GNOME accent colour for the System accent choice.
    pub fn lumina_rgb(&self, accent: [u8; 3]) -> Option<[[u8; 3]; 3]> {
        if self.lumina != LuminaStyle::Colourful {
            return None;
        }
        self.lumina_colours.set().or_else(|| {
            Some(triad(if self.lumina_colours == LuminaColours::Accent {
                accent
            } else {
                self.lumina_colour
            }))
        })
    }

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
            "glass-lumina" => quote(self.lumina.key()),
            "glass-lumina-colours" => quote(self.lumina_colours.key()),
            "glass-lumina-colour" => {
                let [r, g, b] = self.lumina_colour;
                quote(&format!("#{r:02x}{g:02x}{b:02x}"))
            }
            "glass-lumina-intensity" => self.lumina_intensity.to_string(),
            _ => String::new(),
        }
    }

    /// Set one setting from Fusion Desk text (`'STRONG'`, `75`, `true`).
    /// Same rules as planner_glass.py `set_value`: numbers clamp, flags and
    /// names in any case, a colour needs `#rrggbb`. Returns false (and
    /// changes nothing) for an unknown key or a bad value.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let value = value.trim().trim_matches('\'');
        let number = |lo: u16, hi: u16| value.parse::<u16>().ok().map(|n| n.clamp(lo, hi) as u8);
        let flag = || {
            ["true", "false"]
                .iter()
                .find(|f| value.eq_ignore_ascii_case(f))
                .map(|f| *f == "true")
        };
        fn put<T>(slot: &mut T, v: Option<T>) -> bool {
            v.map(|v| *slot = v).is_some()
        }
        match key.trim().trim_start_matches("glass-") {
            "effect" => put(&mut self.effect, GlassEffect::from_key(value)),
            "material" => put(&mut self.material, GlassMaterial::from_key(value)),
            "style" => put(&mut self.style, GlassStyle::from_key(value)),
            "opacity" => put(&mut self.opacity, number(10, 100)),
            "rim" => put(&mut self.rim, flag()),
            "glare" => put(&mut self.glare, flag()),
            "shadow" => put(&mut self.shadow, flag()),
            "light-follows" => put(&mut self.light_follows, flag()),
            "lumina" => put(&mut self.lumina, LuminaStyle::from_key(value)),
            "lumina-colours" => put(&mut self.lumina_colours, LuminaColours::from_key(value)),
            "lumina-colour" => put(
                &mut self.lumina_colour,
                value
                    .strip_prefix('#')
                    .filter(|h| h.len() == 6 && h.is_ascii())
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .map(|n| [(n >> 16) as u8, (n >> 8) as u8, n as u8]),
            ),
            "lumina-intensity" => put(&mut self.lumina_intensity, number(20, 100)),
            "style-strength" => put(&mut self.style_strength, number(0, 200)),
            "shine-strength" => put(&mut self.shine_strength, number(0, 200)),
            "rim-strength" => put(&mut self.rim_strength, number(0, 200)),
            "glare-strength" => put(&mut self.glare_strength, number(0, 200)),
            "shadow-strength" => put(&mut self.shadow_strength, number(0, 200)),
            "light-strength" => put(&mut self.light_strength, number(0, 200)),
            _ => false,
        }
    }

    /// `effect=STRONG` lines, the format of Harbor's glass files.
    pub fn to_text(&self) -> String {
        Self::KEYS
            .iter()
            .chain(Self::LUMINA_KEYS.iter())
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
    /// (`off`, `glossy`, `liquid`, any case) still work.
    pub fn from_text(text: &str) -> Glass {
        Self::parse_checked(text).0
    }

    /// The glass plus the lines that were ignored, and whether any line was
    /// used at all (a file with none follows the scope above, like the
    /// Python apps do).
    pub fn parse_checked(text: &str) -> (Glass, Vec<String>, bool) {
        match text.trim().to_ascii_lowercase().as_str() {
            "off" => return (Glass::OFF, Vec::new(), true),
            "glossy" => {
                let g = Glass {
                    effect: GlassEffect::Soft,
                    material: GlassMaterial::Gloss,
                    style: GlassStyle::Gel,
                    opacity: 100,
                    ..Glass::default()
                };
                return (g, Vec::new(), true);
            }
            "liquid" => {
                let g = Glass {
                    style: GlassStyle::Clear,
                    opacity: 80,
                    ..Glass::default()
                };
                return (g, Vec::new(), true);
            }
            _ => {}
        }
        let mut glass = Glass::default();
        let (mut used, mut ignored) = (false, Vec::new());
        for pair in text
            .split([',', '\n'])
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            match pair.split_once('=') {
                Some((k, v)) if glass.set(k, v) => used = true,
                _ => ignored.push(pair.to_owned()),
            }
        }
        (glass, ignored, used)
    }
}

/// Three neighbouring hues around one colour (-32°, itself, +32°), kept bright
/// enough to read as colour (same as planner_glass.py `triad`).
pub fn triad(rgb: [u8; 3]) -> [[u8; 3]; 3] {
    let [r, g, b] = rgb.map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let mut l = (max + min) / 2.0;
    let d = max - min;
    let mut s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    l = l.clamp(0.45, 0.62);
    s = s.max(0.55);
    let at = |dh: f32| {
        let h = (h + dh).rem_euclid(1.0);
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h * 6.0).rem_euclid(2.0) - 1.0).abs());
        let m = l - c / 2.0;
        let (r, g, b) = match (h * 6.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        [r, g, b].map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
    };
    [at(-32.0 / 360.0), at(0.0), at(32.0 / 360.0)]
}

/// GNOME accent-color names (org.gnome.desktop.interface) as colours.
pub fn gnome_accent(name: &str) -> [u8; 3] {
    match name.trim().trim_matches('\'') {
        "teal" => [0x21, 0x90, 0xa4],
        "green" => [0x3a, 0x94, 0x4a],
        "yellow" => [0xc8, 0x88, 0x00],
        "orange" => [0xed, 0x5b, 0x00],
        "red" => [0xe6, 0x2d, 0x42],
        "pink" => [0xd5, 0x61, 0x99],
        "purple" => [0x91, 0x41, 0xac],
        "slate" => [0x6f, 0x83, 0x96],
        _ => [0x35, 0x84, 0xe4],
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
    /// Lumina Colourful's three colours (None = not Colourful); the backdrop
    /// tints with them, so it matches the accent picked here.
    pub lumina: Option<[[u8; 3]; 3]>,
    /// The window is see-through: the Lumina backdrop thins with the opacity.
    pub see_through: bool,
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

    /// Apply the glass. `system_accent` is GNOME's accent colour, used by
    /// Lumina Colourful's "System accent" (the same colour the Python apps use).
    pub fn with_glass(mut self, glass: Glass, see_through: bool, system_accent: [u8; 3]) -> Self {
        self.glass = glass;
        self.see_through = see_through;
        if !glass.on() {
            return self;
        }
        let dark = self.bg.r() < 128;
        if glass.style == GlassStyle::Planner {
            return self.planner(glass, dark, system_accent);
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
    /// PLANNER palette v3 ("Planner Glass"): one violet-indigo hue family.
    /// Mist backdrop (dark: ink-violet), solid cards (slightly see-through
    /// only while Fusion Desk blurs behind the window), indigo-tinted ink
    /// text, hovers and inputs. Spec: ~/Music/apps/glass-theme/PLANNER-SPEC.md
    fn planner(mut self, glass: Glass, dark: bool, system_accent: [u8; 3]) -> Self {
        let rgba = |r, g, b, a: f32| Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0) as u8);
        let rgb = Color32::from_rgb;
        self.accent = rgb(0x63, 0x66, 0xf1);
        self.on_accent = Color32::WHITE;
        if dark {
            self.card = rgba(40, 35, 76, 0.55);
            self.card_hover = rgba(52, 46, 96, 0.65);
            self.card_border = rgba(0xff, 0xff, 0xff, 0.13);
            self.text = rgb(0xec, 0xeb, 0xf7);
            self.text_muted = rgb(0xa9, 0xa6, 0xc4);
            self.text_faint = rgb(0x6f, 0x6c, 0x8c);
            self.hover = rgba(0xff, 0xff, 0xff, 0.08);
            self.pressed = rgba(0xff, 0xff, 0xff, 0.14);
            self.divider = rgb(0x2c, 0x29, 0x45);
            self.input_bg = rgba(0xff, 0xff, 0xff, 0.06);
            self.input_border = rgba(0xff, 0xff, 0xff, 0.14);
            self.track = rgb(0x33, 0x2f, 0x52);
            self.nav_selected = rgba(0x81, 0x8c, 0xf8, 0.22);
            // Opaque on purpose: code tells dark from light by bg.r(), and
            // egui stores colours premultiplied. The backdrop paints over it.
            self.bg = rgb(27, 31, 69);
            self.nav_bg = rgba(20, 17, 40, 0.40);
            self.tooltip = rgb(0x24, 0x21, 0x3a);
        } else {
            self.card = rgba(0xff, 0xff, 0xff, 0.30);
            self.card_hover = rgba(0xff, 0xff, 0xff, 0.55);
            self.card_border = rgba(0xff, 0xff, 0xff, 0.85);
            self.text = rgb(0x1e, 0x1b, 0x3a);
            self.text_muted = rgb(0x5f, 0x5b, 0x7a);
            self.text_faint = rgb(0x9a, 0x97, 0xb3);
            self.hover = rgba(0xff, 0xff, 0xff, 0.50);
            self.pressed = rgba(0xff, 0xff, 0xff, 0.80);
            self.divider = rgb(0xec, 0xe9, 0xf8);
            self.input_bg = rgba(0xff, 0xff, 0xff, 0.55);
            self.input_border = rgba(0xff, 0xff, 0xff, 0.90);
            self.track = rgb(0xe4, 0xe0, 0xff);
            self.nav_selected = rgba(0x63, 0x66, 0xf1, 0.12);
            self.bg = rgb(220, 224, 255);
            self.nav_bg = rgba(0xff, 0xff, 0xff, 0.30);
            self.tooltip = Color32::WHITE;
        }
        // Lumina Colourful: the main hue becomes the accent (icons, switches,
        // selection); the backdrop tint is painted by widgets::planner_backdrop
        self.lumina = glass.lumina_rgb(system_accent);
        if let Some([_, [r, g, b], _]) = self.lumina {
            self.accent = rgb(r, g, b);
            self.nav_selected = rgba(r, g, b, if dark { 0.24 } else { 0.14 });
        }
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
                lumina: None,
                see_through: false,
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
                lumina: None,
                see_through: false,
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
                lumina: None,
                see_through: false,
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
                lumina: None,
                see_through: false,
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

#[cfg(test)]
mod glass_parse_tests {
    use super::*;

    // Same rules as planner_glass.py: a file written by one app reads the same in all.
    #[test]
    fn values_clamp_and_check() {
        let (g, ignored, used) = Glass::parse_checked(
            "opacity=300\nrim=True\nlumina-colour=8b5cf6\nlumina-colour=#+0_1_2\nstyle=planner\nbogus",
        );
        assert!(used);
        assert_eq!(g.opacity, 100);
        assert!(g.rim);
        assert_eq!(g.style, GlassStyle::Planner);
        assert_eq!(g.lumina_colour, Glass::default().lumina_colour);
        assert_eq!(ignored.len(), 3);
    }

    #[test]
    fn legacy_words_any_case() {
        assert_eq!(Glass::from_text("GLOSSY").material, GlassMaterial::Gloss);
        assert_eq!(Glass::from_text("Off"), Glass::OFF);
    }

    #[test]
    fn garbage_is_not_used() {
        let (_, ignored, used) = Glass::parse_checked("nonsense words");
        assert!(!used);
        assert_eq!(ignored, vec!["nonsense words".to_owned()]);
    }

    #[test]
    fn text_round_trip() {
        let g = Glass {
            style: GlassStyle::Planner,
            lumina: LuminaStyle::Colourful,
            lumina_colour: [0x12, 0xab, 0xef],
            ..Glass::default()
        };
        assert_eq!(Glass::from_text(&g.to_text()), g);
    }
}
