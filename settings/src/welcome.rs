//! First-run welcome, feature tour and look setup (like Harbor Files).
//!
//! Shown once, the first time the window opens. Closing it at any step never
//! blocks the app: the look chosen on the way is put back unless the user
//! finished the guide. Personalisation > "Show the welcome guide" reopens it.

use eframe::egui::{
    self, Align2, Color32, Context, CursorIcon, Id, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{regular, semibold, Mode, Palette, ACCENTS};
use crate::widgets;

const STEPS: [&str; 4] = ["Welcome", "Tour", "Look", "Finish"];

/// (icon, title, text, chips)
const FEATURES: [(Icon, &str, &str, [&str; 3]); 5] = [
    (
        Icon::System,
        "Everything in one place",
        "Display, sound, network, Bluetooth, power and more are changed right here. \
         Settings never sends you off to another app.",
        ["Display", "Sound", "Network"],
    ),
    (
        Icon::Apps,
        "Apps and updates",
        "My Apps, Updates, Library and All Apps live on the Apps page. Ubuntu updates \
         install with one password prompt.",
        ["Install", "Update", "Uninstall"],
    ),
    (
        Icon::Storage,
        "Free up space",
        "The disk bar on the System page shows what is left. Free Up Space removes \
         package caches and leftovers; files go to the Trash first.",
        ["Disk bar", "Cleaner", "Trash"],
    ),
    (
        Icon::Search,
        "Find any setting",
        "Press Ctrl+F and type: every page is searched as you type. Esc goes back.",
        ["Ctrl+F", "Live search", "Esc"],
    ),
    (
        Icon::Personalise,
        "Make it yours",
        "Light or dark mode, an accent colour and your wallpaper. Apply the look to \
         Ubuntu too with one click.",
        ["Mode", "Accent", "Wallpaper"],
    ),
];

pub struct Welcome {
    step: usize,
    tour: usize,
    saved_mode: Mode,
    saved_accent: Color32,
    saved_follow: bool,
    apply_ubuntu: bool,
}

impl Welcome {
    pub fn new(app: &App) -> Self {
        // WIN11_SETTINGS_WELCOME=<step>[.<tour page>] opens a given page (screenshots).
        let start = std::env::var("WIN11_SETTINGS_WELCOME").unwrap_or_default();
        let mut parts = start.split('.').map(|p| p.parse::<usize>().unwrap_or(0));
        let step = parts.next().unwrap_or(0).min(STEPS.len() - 1);
        let tour = parts.next().unwrap_or(0).min(FEATURES.len() - 1);
        Self {
            step,
            tour,
            saved_mode: app.mode,
            saved_accent: app.accent,
            saved_follow: app.follow_system,
            apply_ubuntu: false,
        }
    }
}

fn marker() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });
    base.join("win11-settings").join("onboarded")
}

/// Whether to open the guide at start: first run only, never in screenshot
/// or test runs unless WIN11_SETTINGS_WELCOME=1 asks for it.
pub fn show_at_start() -> bool {
    if std::env::var("WIN11_SETTINGS_WELCOME").is_ok() {
        return true;
    }
    if std::env::var("WIN11_SETTINGS_SCREENSHOT").is_ok()
        || std::env::var("WIN11_SETTINGS_NO_WELCOME").is_ok()
    {
        return false;
    }
    !marker().exists()
}

fn mark_done() {
    let path = marker();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, b"1\n");
}

fn text(ui: &mut Ui, s: &str, size: f32, color: Color32) {
    ui.label(RichText::new(s).font(regular(size)).color(color));
}

fn title(ui: &mut Ui, s: &str, size: f32, color: Color32) {
    ui.label(RichText::new(s).font(semibold(size)).color(color));
}

fn chip(ui: &mut Ui, pal: &Palette, s: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(s.to_owned(), regular(12.0), pal.text);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(20.0, 10.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 99, pal.accent.gamma_multiply(0.14));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, pal.text);
}

pub fn show(app: &mut App, ctx: &Context, pal: &Palette) {
    let Some(mut w) = app.welcome.take() else {
        return;
    };
    let mut close = false;
    let mut finish = false;

    // Dim the window behind and swallow its clicks.
    let screen = ctx.viewport_rect();
    egui::Area::new(Id::new("welcome-dim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0, Color32::from_black_alpha(110));
        });

    let size = Vec2::new(
        (screen.width() - 48.0).clamp(420.0, 720.0),
        (screen.height() - 64.0).clamp(380.0, 520.0),
    );
    egui::Window::new("Welcome")
        .id(Id::new("welcome-dialog"))
        .order(egui::Order::Foreground)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .fixed_size(size)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(
            egui::Frame::new()
                .fill(crate::widgets::solid(pal.card))
                .stroke(Stroke::new(1.0, pal.card_border))
                .corner_radius(pal.corner(10))
                .inner_margin(Margin::same(26)),
        )
        .show(ctx, |ui| {
            ui.set_min_size(size);
            ui.set_max_size(size);
            header(ui, pal, &w, &mut close);
            ui.add_space(14.0);
            let body_h = size.y - 34.0 - 14.0 - 52.0;
            ui.allocate_ui(Vec2::new(size.x, body_h), |ui| {
                ui.set_min_height(body_h);
                match w.step {
                    0 => welcome_page(ui, pal, &mut w, &mut close),
                    1 => tour_page(ui, pal, &mut w),
                    2 => look_page(app, ui, pal),
                    _ => finish_page(app, ui, pal, &mut w),
                }
            });
            footer(ui, pal, &mut w, &mut finish);
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if finish {
        if w.apply_ubuntu {
            app.apply_appearance_to_desktop();
        }
        mark_done();
        app.notify("Welcome to Settings");
    } else if close {
        app.mode = w.saved_mode;
        app.accent = w.saved_accent;
        app.follow_system = w.saved_follow;
        mark_done();
    } else {
        app.welcome = Some(w);
    }
}

fn header(ui: &mut Ui, pal: &Palette, w: &Welcome, close: &mut bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (i, name) in STEPS.iter().enumerate() {
            let current = i == w.step;
            let done = i < w.step;
            let color = if current {
                pal.on_accent
            } else if done {
                pal.text
            } else {
                pal.text_faint
            };
            let galley = ui
                .painter()
                .layout_no_wrap((*name).to_owned(), semibold(12.0), color);
            let (rect, _) =
                ui.allocate_exact_size(galley.size() + Vec2::new(22.0, 12.0), Sense::hover());
            if current {
                ui.painter().rect_filled(rect, 99, pal.accent);
            } else if done {
                ui.painter().rect_filled(rect, 99, pal.hover);
            }
            ui.painter()
                .galley(rect.center() - galley.size() / 2.0, galley, color);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(rect, pal.corner(5), pal.hover);
            }
            let c = rect.center();
            let s = 5.0;
            let stroke = Stroke::new(1.2, pal.text_muted);
            ui.painter()
                .line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(s, -s), c + Vec2::new(-s, s)], stroke);
            if resp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Close")
                .clicked()
            {
                *close = true;
            }
        });
    });
}

fn welcome_page(ui: &mut Ui, pal: &Palette, w: &mut Welcome, close: &mut bool) {
    ui.vertical_centered(|ui| {
        ui.add_space(26.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(76.0), Sense::hover());
        widgets::icon_tile(
            ui.painter(),
            rect,
            Icon::System,
            pal.accent,
            pal.on_accent,
            pal,
        );
        ui.add_space(18.0);
        title(ui, "Welcome to Settings", 30.0, pal.text);
        ui.add_space(6.0);
        text(
            ui,
            "Your Ubuntu settings, apps and updates in one Windows 11-style window.",
            15.0,
            pal.text_muted,
        );
        ui.add_space(26.0);
        if widgets::button(ui, pal, pal.accent, "Take the tour", Some(Icon::Sparkle)).clicked() {
            w.step = 1;
        }
        ui.add_space(8.0);
        if widgets::ghost_button(ui, pal, "Skip to setup", None).clicked() {
            w.step = 2;
        }
        ui.add_space(8.0);
        let later = ui.add(
            egui::Label::new(
                RichText::new("Start with the current settings")
                    .font(regular(13.0))
                    .color(pal.text_muted),
            )
            .sense(Sense::click()),
        );
        if later.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *close = true;
        }
    });
}

fn tour_page(ui: &mut Ui, pal: &Palette, w: &mut Welcome) {
    let (icon, head, body, chips) = FEATURES[w.tour.min(FEATURES.len() - 1)];
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
        widgets::icon_tile(ui.painter(), rect, icon, pal.accent, pal.on_accent, pal);
        ui.add_space(16.0);
        title(ui, head, 22.0, pal.text);
        ui.add_space(8.0);
        ui.set_max_width(520.0);
        ui.label(
            RichText::new(body)
                .font(regular(14.5))
                .color(pal.text_muted),
        );
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let total: f32 = chips
                .iter()
                .map(|c| {
                    ui.painter()
                        .layout_no_wrap((*c).to_owned(), regular(12.0), pal.text)
                        .size()
                        .x
                        + 28.0
                })
                .sum();
            ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
            ui.spacing_mut().item_spacing.x = 8.0;
            for c in chips {
                chip(ui, pal, c);
            }
        });
        ui.add_space(22.0);
        // Page dots: click to jump.
        ui.horizontal(|ui| {
            let n = FEATURES.len() as f32;
            ui.add_space(((ui.available_width() - n * 18.0) / 2.0).max(0.0));
            for i in 0..FEATURES.len() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, 12.0), Sense::click());
                let on = i == w.tour;
                let fill = if on { pal.accent } else { pal.text_faint };
                let r = if on {
                    Rect::from_center_size(rect.center(), Vec2::new(14.0, 6.0))
                } else {
                    Rect::from_center_size(rect.center(), Vec2::splat(6.0))
                };
                ui.painter().rect_filled(r, 3, fill);
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    w.tour = i;
                }
            }
        });
    });
}

fn look_page(app: &mut App, ui: &mut Ui, pal: &Palette) {
    ui.vertical_centered(|ui| {
        title(ui, "Choose your look", 22.0, pal.text);
        ui.add_space(4.0);
        text(
            ui,
            "It changes right away. You can change it later in Personalisation.",
            14.0,
            pal.text_muted,
        );
    });
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        let card = Vec2::new(150.0, 96.0);
        ui.spacing_mut().item_spacing.x = 14.0;
        ui.add_space(((ui.available_width() - 3.0 * card.x - 28.0) / 2.0).max(0.0));
        for (label, note, which) in [
            ("Light", "Bright surfaces", Some(Mode::Light)),
            ("Dark", "Low-light surfaces", Some(Mode::Dark)),
            ("Use system", "Follow Ubuntu", None),
        ] {
            let preview_mode = which.unwrap_or(app.system_mode);
            let selected = match which {
                Some(m) => !app.follow_system && app.mode == m,
                None => app.follow_system,
            };
            let (rect, resp) = ui.allocate_exact_size(card, Sense::click());
            let (bg, fg) = if preview_mode == Mode::Light {
                (
                    Color32::from_rgb(0xF3, 0xF3, 0xF3),
                    Color32::from_rgb(0x20, 0x20, 0x20),
                )
            } else {
                (Color32::from_rgb(0x20, 0x20, 0x20), Color32::WHITE)
            };
            ui.painter().rect_filled(rect, pal.corner(8), bg);
            // Tiny window sketch with the accent.
            let bar = Rect::from_min_size(rect.min + Vec2::new(14.0, 14.0), Vec2::new(46.0, 8.0));
            ui.painter().rect_filled(bar, pal.corner(4), app.accent);
            ui.painter().rect_stroke(
                rect,
                8,
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
            ui.painter().text(
                Pos2::new(rect.center().x, rect.bottom() - 32.0),
                Align2::CENTER_CENTER,
                label,
                semibold(13.0),
                fg,
            );
            ui.painter().text(
                Pos2::new(rect.center().x, rect.bottom() - 16.0),
                Align2::CENTER_CENTER,
                note,
                regular(11.0),
                fg.gamma_multiply(0.75),
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                match which {
                    Some(m) => {
                        app.follow_system = false;
                        app.mode = m;
                    }
                    None => app.use_system_appearance(),
                }
            }
        }
    });
    ui.add_space(22.0);
    ui.vertical_centered(|ui| title(ui, "Accent colour", 14.0, pal.text));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_space(((ui.available_width() - ACCENTS.len() as f32 * 42.0) / 2.0).max(0.0));
        for (name, color) in ACCENTS {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
            ui.painter().circle_filled(rect.center(), 12.5, color);
            if app.accent == color {
                ui.painter()
                    .circle_stroke(rect.center(), 15.5, Stroke::new(2.0, pal.text));
            }
            if resp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(name)
                .clicked()
            {
                app.follow_system = false;
                app.accent = color;
            }
        }
    });
}

fn finish_page(app: &mut App, ui: &mut Ui, pal: &Palette, w: &mut Welcome) {
    ui.vertical_centered(|ui| {
        title(ui, "You're all set", 22.0, pal.text);
        ui.add_space(4.0);
        text(ui, "Check your choices, then start.", 14.0, pal.text_muted);
    });
    ui.add_space(18.0);
    let mode = if app.follow_system {
        "Use system (follows Ubuntu)"
    } else if app.mode == Mode::Light {
        "Light"
    } else {
        "Dark"
    };
    let accent = if app.follow_system {
        "Ubuntu accent"
    } else {
        ACCENTS
            .iter()
            .find(|(_, c)| *c == app.accent)
            .map(|(n, _)| *n)
            .unwrap_or("Custom")
    };
    widgets::card(ui, pal, |ui| {
        for (icon, label, value) in [
            (Icon::Moon, "Mode", mode),
            (Icon::Personalise, "Accent", accent),
        ] {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 44.0), Sense::hover());
                icons::paint(
                    ui.painter(),
                    icon,
                    Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
                    pal.text_muted,
                );
                ui.add_space(8.0);
                title(ui, label, 14.0, pal.text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(16.0);
                    text(ui, value, 14.0, pal.text_muted);
                });
            });
        }
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(40.0, 44.0), Sense::click());
            let knob = Rect::from_center_size(rect.center(), Vec2::new(40.0, 20.0));
            widgets::paint_switch(
                ui.painter(),
                knob,
                if w.apply_ubuntu { 1.0 } else { 0.0 },
                pal,
                pal.accent,
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                w.apply_ubuntu = !w.apply_ubuntu;
            }
            ui.add_space(10.0);
            title(ui, "Use this look for Ubuntu too", 14.0, pal.text);
        });
    });
    ui.add_space(12.0);
    text(
        ui,
        "Reopen this guide any time from Personalisation.",
        12.5,
        pal.text_faint,
    );
}

fn footer(ui: &mut Ui, pal: &Palette, w: &mut Welcome, finish: &mut bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if w.step > 0 && widgets::ghost_button(ui, pal, "Back", Some(Icon::Back)).clicked() {
            if w.step == 1 && w.tour > 0 {
                w.tour -= 1;
            } else {
                w.step -= 1;
            }
        }
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| match w.step {
                0 => {}
                3 => {
                    if widgets::button(
                        ui,
                        pal,
                        pal.accent,
                        "Start using Settings",
                        Some(Icon::Check),
                    )
                    .clicked()
                    {
                        *finish = true;
                    }
                }
                _ => {
                    if widgets::button(ui, pal, pal.accent, "Next", None).clicked() {
                        if w.step == 1 && w.tour + 1 < FEATURES.len() {
                            w.tour += 1;
                        } else {
                            w.step += 1;
                        }
                    }
                    if w.step == 1 && widgets::ghost_button(ui, pal, "Skip tour", None).clicked() {
                        w.step = 2;
                    }
                }
            },
        );
    });
}
