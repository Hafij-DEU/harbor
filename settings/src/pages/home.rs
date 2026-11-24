//! Home page (Windows 11 and Windows 10 designs) and quick settings.

use eframe::egui::{
    Align, Align2, Color32, CursorIcon, Id, Layout, Pos2, Rect, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2,
};

use crate::app::{App, Detail, Page, View};
use crate::icons::Icon;
use crate::system::{self};
use crate::theme::{regular, semibold, with_alpha, Palette};
use crate::widgets::{self};

/// A toggle on the Windows 10 home page.
#[derive(Clone, Copy)]
pub(crate) enum Quick {
    Wifi,
    Bluetooth,
    NightLight,
    Focus,
    Mute,
    Dark,
}

impl Quick {
    pub(crate) const ALL: [Quick; 6] = [
        Quick::Wifi,
        Quick::Bluetooth,
        Quick::NightLight,
        Quick::Focus,
        Quick::Mute,
        Quick::Dark,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Quick::Wifi => "Wi-Fi",
            Quick::Bluetooth => "Bluetooth",
            Quick::NightLight => "Night light",
            Quick::Focus => "Do not disturb",
            Quick::Mute => "Mute",
            Quick::Dark => "Dark mode",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Quick::Wifi => Icon::Wifi,
            Quick::Bluetooth => Icon::Bluetooth,
            Quick::NightLight => Icon::Sun,
            Quick::Focus => Icon::Bell,
            Quick::Mute => Icon::Sound,
            Quick::Dark => Icon::Moon,
        }
    }

    pub(crate) fn is_on(self, app: &App) -> bool {
        match self {
            Quick::Wifi => app.info.wifi_enabled,
            Quick::Bluetooth => app.info.bluetooth_on,
            Quick::NightLight => app.info.night_light,
            Quick::Focus => !app.info.notifications,
            Quick::Mute => app.info.muted,
            Quick::Dark => app.mode == crate::theme::Mode::Dark,
        }
    }

    pub(crate) fn toggle(self, app: &mut App) {
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
pub(crate) fn home_extras(cat: crate::app::Cat10) -> &'static [(&'static str, Icon, View)] {
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
