//! Personalisation: theme, accent, wallpaper, cursor, glass effect.

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use crate::app::App;
use crate::icons::Icon;
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self, Right, ROW_PAD};

use super::*;

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

    // Accent or Colourful icons, for the whole app.
    spacer(ui, 12.0);
    widgets::card(ui, &pal, |ui| {
        use crate::theme::Look;
        let options: Vec<(String, String)> = Look::ALL
            .iter()
            .map(|l| (l.key().to_owned(), l.label().to_owned()))
            .collect();
        let picked = super::combo_row(
            ui,
            &pal,
            Icon::Sparkle,
            "Colour style",
            "Accent: every icon in the accent colour. Colourful: each icon its own colour",
            "look",
            &options,
            app.look.key(),
            190.0,
        );
        if let Some(look) = picked.as_deref().and_then(Look::from_key) {
            app.set_look(look);
        }
        ui.add_space(10.0);
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    Light,
    Dark,
    System,
}

/// Personalisation › Glass effects: Fusion Desk's glass settings for the
/// whole desktop ("Global"), for every Harbor app, and for each app. A
/// scope can follow the one above it or have its own values.
/// Ready-made glass looks (Settings › Personalisation › Glass effects).
pub(crate) const GLASS_PRESETS: [(&str, &str); 13] = [
    // Daily Routine Planner look (PLANNER-SPEC.md): violet glass cards on a
    // lavender backdrop, indigo accent; opacity 100 keeps windows solid.
    ("Lumina Glass", "effect=STRONG,material=GLASS,style=PLANNER,opacity=100,rim=true,glare=false,shadow=true,light-follows=false,style-strength=100,shine-strength=60,rim-strength=100,glare-strength=100,shadow-strength=100,light-strength=100"),
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

pub(crate) fn glass_section(app: &mut App, ui: &mut Ui, pal: &Palette) {
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
        // Two choices only: the desktop-wide glass, or one app's own glass.
        // "Per app" opens a second line with the apps (All Harbor apps first).
        let per_app = scope != GlassScope::Global;
        glass_row(
            ui,
            pal,
            "Apply to",
            "Global is the desktop: the same settings as Fusion Desk › Appearance",
            |ui| {
                if segment(ui, pal, "Global", !per_app) {
                    pick_scope = Some(GlassScope::Global);
                }
                if segment(ui, pal, "Per app", per_app) && !per_app {
                    pick_scope = Some(GlassScope::ALL[2]);
                }
            },
        );
        if per_app {
            glass_row(
                ui,
                pal,
                "App",
                "Each app follows the glass above it or gets its own",
                |ui| {
                    for s in &GlassScope::ALL[2..] {
                        if segment(ui, pal, s.label(), *s == scope) {
                            pick_scope = Some(*s);
                        }
                    }
                },
            );
        }
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
    });
    if own.is_some() {
        // Presets and the on/off switch first; the fine-tuning lives in a
        // collapsed Advanced card so the page reads top to bottom.
        spacer(ui, 10.0);
        widgets::card(ui, pal, |ui| {
            glass_row(
                ui,
                pal,
                "Presets",
                "Start from a ready-made look; fine-tune it in Advanced settings",
                |ui| {
                    for (name, text) in GLASS_PRESETS {
                        // a preset keeps the Lumina options (style, colours)
                        let preset = crate::theme::Glass {
                            lumina: values.lumina,
                            lumina_colours: values.lumina_colours,
                            lumina_colour: values.lumina_colour,
                            lumina_intensity: values.lumina_intensity,
                            ..crate::theme::Glass::from_text(text)
                        };
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
        });
        if values.on() && values.style == crate::theme::GlassStyle::Planner {
            // Lumina's own options, global or per app like every glass value
            spacer(ui, 10.0);
            widgets::card(ui, pal, |ui| {
                glass_row(
                    ui,
                    pal,
                    "Lumina style",
                    "Calm: the indigo glass. Colourful: a few headings, icons and selections in colour",
                    |ui| {
                        for l in crate::theme::LuminaStyle::ALL {
                            if segment(ui, pal, l.label(), values.lumina == *l) {
                                values.lumina = *l;
                            }
                        }
                    },
                );
                if values.lumina != crate::theme::LuminaStyle::Colourful {
                    return;
                }
                widgets::divider(ui, pal);
                glass_row(
                    ui,
                    pal,
                    "Colours",
                    "A colour set, the system accent colour or your own colour",
                    |ui| {
                        for c in crate::theme::LuminaColours::ALL {
                            if segment(ui, pal, c.label(), values.lumina_colours == *c) {
                                values.lumina_colours = *c;
                            }
                        }
                    },
                );
                if values.lumina_colours == crate::theme::LuminaColours::Custom {
                    widgets::divider(ui, pal);
                    glass_row(
                        ui,
                        pal,
                        "Custom colour",
                        "The colours sweep around this one",
                        |ui| {
                            let mut rgb = values.lumina_colour;
                            if egui::widgets::color_picker::color_edit_button_srgb(ui, &mut rgb)
                                .changed()
                            {
                                values.lumina_colour = rgb;
                            }
                        },
                    );
                }
                widgets::divider(ui, pal);
                let mut t = (values.lumina_intensity as f32 - 20.0) / 80.0;
                if slider_row(
                    ui,
                    pal,
                    Icon::Sparkle,
                    "Colour intensity",
                    format!("{} %", values.lumina_intensity),
                    &mut t,
                ) {
                    values.lumina_intensity = ((20.0 + t * 80.0) / 5.0).round() as u8 * 5;
                }
            });
        }
        if values.on() {
            let id = egui::Id::new("hb-glass-advanced");
            let mut open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
            spacer(ui, 10.0);
            widgets::card(ui, pal, |ui| {
                if widgets::row(
                    ui,
                    pal,
                    pal.accent,
                    Icon::Sparkle,
                    "Advanced settings",
                    "Material, style, opacity, rim, glare, shadow and pointer light",
                    Right::AccentText(if open { "Hide" } else { "Show" }),
                )
                .clicked
                {
                    open = !open;
                }
                if !open {
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
            ui.ctx().data_mut(|d| d.insert_temp(id, open));
        }
    }
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
pub(crate) fn strength_row(ui: &mut Ui, pal: &Palette, title: &str, value: &mut u8) {
    let mut t = *value as f32 / 200.0;
    let sub = format!("{} % · 100 % is normal", value);
    if slider_row(ui, pal, Icon::Sparkle, title, sub, &mut t) {
        *value = ((t * 200.0 / 5.0).round() * 5.0) as u8;
    }
}

pub(crate) fn glass_row(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    sub: &str,
    choices: impl FnOnce(&mut Ui),
) {
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
