//! Window chrome: title bar, sidebars, page routing, shortcuts, resize edges.

use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, Context, Id, Layout, Margin, Pos2, Rect, ScrollArea, Sense,
    Stroke, Ui, UiBuilder, Vec2, ViewportCommand,
};

use super::*;
use crate::icons::Icon;
use crate::pages;
use crate::theme::{regular, semibold, Palette};
use crate::widgets::{self, avatar, caption_button, nav_item, Caption};

pub(crate) fn title_height(design: crate::theme::Design) -> f32 {
    if design == crate::theme::Design::Win10 {
        TITLE_H_10
    } else {
        TITLE_H
    }
}

impl App {
    pub fn go(&mut self, view: View) {
        self.view = view;
        self.search.clear();
    }

    /// Put the page on screen at the top of Recently accessed (saved when
    /// it changes). Called every frame, so every way of opening a page counts.
    pub(crate) fn note_recent(&mut self) {
        const KEEP: usize = 12;
        if self.view == View::Home || self.recent.first() == Some(&self.view) {
            return;
        }
        let view = self.view;
        self.recent.retain(|v| *v != view);
        self.recent.insert(0, view);
        self.recent.truncate(KEEP);
        system::save_recent(&self.recent);
    }

    pub fn back(&mut self) {
        match self.view {
            View::Detail(page, _) => self.view = View::Page(page),
            // Windows 10 returns to the tile home screen; Windows 11 keeps
            // its current page (there is no home screen in that design).
            View::Page(_) if self.design == crate::theme::Design::Win10 => {
                self.search.clear();
                self.view = View::Home;
            }
            View::Page(_) | View::Home => {}
        }
    }

    pub(crate) fn handle_shortcuts(&mut self, ctx: &Context) {
        // The welcome guide handles its own keys (Esc closes it), and a
        // Keyboard Manager key box takes the next key press itself.
        if self.welcome.is_some() || self.keyboard.wants_keys() {
            return;
        }
        let (escape, find) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.modifiers.command && i.key_pressed(egui::Key::F),
            )
        });
        if find {
            self.focus_search = true;
        }
        if escape {
            if let Some(dialog) = &self.confirm {
                if !dialog.busy {
                    self.confirm = None;
                }
            } else if self.wifi_connect.is_some() {
                self.wifi_connect = None;
            } else if self.text_prompt.is_some() {
                if !self.text_prompt.as_ref().map(|d| d.busy).unwrap_or(false) {
                    self.text_prompt = None;
                }
            } else if self.rename.is_some() {
                if !self.rename.as_ref().map(|d| d.busy).unwrap_or(false) {
                    self.rename = None;
                }
            } else if !self.search.is_empty() {
                self.search.clear();
            } else {
                self.back();
            }
        }
    }

    pub(crate) fn titlebar(&mut self, ui: &mut Ui, ctx: &Context, pal: &Palette, rect: Rect) {
        let buttons_w = 46.0 * 3.0;
        // Design and light/dark switches sit just left of the caption buttons.
        let quick_w = 8.0 + 110.0 + 6.0 + 30.0 + 10.0;
        let drag_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right() - buttons_w - quick_w, rect.bottom()),
        );
        let response = ui.interact(drag_rect, Id::new("titlebar-drag"), Sense::click_and_drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if response.double_clicked() {
            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }

        if pal.design == crate::theme::Design::Win10 {
            // Windows 10: a small app tile, the window title and the search
            // box in the title bar, on a white (or black) strip.
            // On the home page it takes the header band's colour so the two
            // read as one block.
            let home = matches!(self.view, View::Home);
            if home {
                let (top, _) = crate::widgets::home10_header(pal);
                let r = self.frame_r;
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius {
                        nw: r,
                        ne: r,
                        sw: 0,
                        se: 0,
                    },
                    top,
                );
            }
            let tile = Rect::from_center_size(
                Pos2::new(rect.left() + 20.0, rect.center().y),
                Vec2::splat(16.0),
            );
            ui.painter().rect_filled(tile, 0, pal.accent);
            crate::icons::paint_square(
                ui.painter(),
                Icon::System,
                tile.shrink(3.0),
                Color32::WHITE,
            );
            ui.painter().text(
                Pos2::new(rect.left() + 34.0, rect.center().y),
                Align2::LEFT_CENTER,
                "Settings",
                regular(12.0),
                pal.text,
            );
            if !home {
                ui.painter().rect_filled(
                    Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 1.0), rect.max),
                    0,
                    pal.divider,
                );
            }
        } else {
            let dot = Pos2::new(rect.left() + 18.0, rect.center().y);
            ui.painter().circle_filled(dot, 2.6, pal.accent);
            ui.painter().text(
                Pos2::new(rect.left() + 28.0, rect.center().y),
                Align2::LEFT_CENTER,
                "Settings",
                regular(12.5),
                pal.text_muted,
            );
        }

        self.title_switches(ui, ctx, pal, rect, buttons_w + 10.0);

        let max = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        // Caption buttons fill the title bar and touch the window edge, so the
        // very corner of the screen still hits Close.
        let controls = Rect::from_min_size(
            Pos2::new(rect.right() - buttons_w, rect.top()),
            Vec2::new(buttons_w, rect.height()),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(controls)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if caption_button(ui, pal, Caption::Minimize, pal.accent, 0).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                }
                let glyph = if max {
                    Caption::Restore
                } else {
                    Caption::Maximize
                };
                if caption_button(ui, pal, glyph, pal.accent, 0).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
                }
                if caption_button(ui, pal, Caption::Close, pal.accent, self.frame_r).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            },
        );
    }

    /// "Win 10 | Win 11" design switch and a light/dark button in the title
    /// bar, `right_gap` points left of the window's right edge.
    pub(crate) fn title_switches(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        pal: &Palette,
        rect: Rect,
        right_gap: f32,
    ) {
        use crate::theme::{Design, Mode};
        let h = 24.0;
        let cy = rect.center().y;

        // Light/dark: shows the mode you switch to.
        let mode_rect = Rect::from_center_size(
            Pos2::new(rect.right() - right_gap - 15.0, cy),
            Vec2::new(30.0, h),
        );
        let resp = ui
            .interact(mode_rect, Id::new("title-mode"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let dark = self.mode == Mode::Dark;
        let resp = resp.on_hover_text(if dark {
            "Switch to light mode"
        } else {
            "Switch to dark mode"
        });
        if resp.hovered() {
            ui.painter()
                .rect_filled(mode_rect, pal.corner(4), pal.hover);
        }
        crate::icons::paint(
            ui.painter(),
            if dark { Icon::Sun } else { Icon::Moon },
            Rect::from_center_size(mode_rect.center(), Vec2::splat(15.0)),
            pal.text_muted,
        );
        if resp.clicked() {
            self.follow_system = false;
            self.mode = if dark { Mode::Light } else { Mode::Dark };
            ctx.request_repaint();
        }

        // Design: a two-part pill.
        let pill = Rect::from_min_size(
            Pos2::new(mode_rect.left() - 6.0 - 110.0, cy - h / 2.0),
            Vec2::new(110.0, h),
        );
        let radius = pal.corner(12);
        ui.painter().rect_filled(pill, radius, pal.hover);
        for (i, (design, label)) in [(Design::Win10, "Win 10"), (Design::Win11, "Win 11")]
            .into_iter()
            .enumerate()
        {
            let seg = Rect::from_min_size(
                Pos2::new(pill.left() + i as f32 * 55.0, pill.top()),
                Vec2::new(55.0, h),
            )
            .shrink(2.0);
            let resp = ui
                .interact(seg, Id::new(("title-design", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("{} design", design.label()));
            let active = self.design == design;
            if active {
                ui.painter().rect_filled(seg, pal.corner(10), pal.accent);
            } else if resp.hovered() {
                ui.painter().rect_filled(seg, pal.corner(10), pal.pressed);
            }
            let color = if active {
                pal.on_accent
            } else {
                pal.text_muted
            };
            ui.painter().text(
                seg.center(),
                Align2::CENTER_CENTER,
                label,
                if active {
                    semibold(11.5)
                } else {
                    regular(11.5)
                },
                color,
            );
            if resp.clicked() && !active {
                self.set_design(design);
                ctx.request_repaint();
            }
        }
    }

    pub(crate) fn sidebar(&mut self, ui: &mut Ui, pal: &Palette, rect: Rect) {
        if pal.design == crate::theme::Design::Win10 {
            return self.sidebar10(ui, pal, rect);
        }
        let margin = 14.0;
        let inner = Rect::from_min_max(
            Pos2::new(rect.left() + margin, rect.top() + 2.0),
            Pos2::new(rect.right() - margin, rect.bottom()),
        );
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            ui.set_width(inner.width());

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                avatar(ui, pal, &self.info.full_name, 40.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        egui::RichText::new(&self.info.full_name)
                            .font(semibold(14.0))
                            .color(pal.text),
                    );
                    ui.label(
                        egui::RichText::new(if self.info.is_admin {
                            "Local account · Administrator"
                        } else {
                            "Local account · Standard user"
                        })
                        .font(regular(11.5))
                        .color(pal.text_faint),
                    );
                });
            });
            ui.add_space(14.0);

            let (search_response, _) = widgets::search_box(
                ui,
                pal,
                &mut self.search,
                std::mem::take(&mut self.focus_search),
                "Find a setting",
            );
            let _ = search_response;

            ui.add_space(10.0);
            let height = ui.available_height() - 6.0;
            ScrollArea::vertical()
                .id_salt("nav")
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, |ui| {
                    ui.set_width(inner.width());
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for page in Page::ALL {
                        let selected = self.view.page() == page;
                        if nav_item(ui, pal, pal.accent, page.icon(), page.label(), selected)
                            .clicked()
                        {
                            self.go(View::Page(page));
                        }
                    }
                    ui.add_space(12.0);
                });
        });
    }

    /// Windows 10 navigation pane: a flat, full-bleed list on the grey pane,
    /// with the account strip pinned to the bottom.
    pub(crate) fn sidebar10(&mut self, ui: &mut Ui, pal: &Palette, rect: Rect) {
        ui.painter().rect_filled(rect, 0, pal.nav_bg);
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::new(rect.right() - 1.0, rect.top()), rect.max),
            0,
            pal.card_border,
        );
        let account_h = 62.0;
        // "Home" sits above the search box, as in Windows 10.
        let home_rect = Rect::from_min_size(
            rect.min + Vec2::new(0.0, 6.0),
            Vec2::new(rect.width() - 1.0, 40.0),
        );
        ui.scope_builder(UiBuilder::new().max_rect(home_rect), |ui| {
            ui.set_width(home_rect.width());
            if nav_item(ui, pal, pal.accent, Icon::Home, "Home", false).clicked() {
                self.go(View::Home);
            }
        });
        let search_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, home_rect.bottom() + 6.0),
            Vec2::new(rect.width() - 26.0, 36.0),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(search_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.set_width(search_rect.width());
                widgets::search_box(
                    ui,
                    pal,
                    &mut self.search,
                    std::mem::take(&mut self.focus_search),
                    "Find a setting",
                );
            },
        );
        let list_rect = Rect::from_min_max(
            Pos2::new(rect.left(), search_rect.bottom() + 10.0),
            Pos2::new(rect.right() - 1.0, rect.bottom() - account_h - 6.0),
        );
        let cat = Cat10::of(self.view);
        ui.scope_builder(UiBuilder::new().max_rect(list_rect), |ui| {
            ui.set_width(list_rect.width());
            ScrollArea::vertical()
                .id_salt("nav10")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(list_rect.width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    match cat {
                        Some(cat) => {
                            // The category name heads its own sub-pages.
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    egui::RichText::new(cat.title())
                                        .font(semibold(15.0))
                                        .color(pal.text),
                                );
                            });
                            ui.add_space(10.0);
                            for (label, icon, view) in cat.items() {
                                let selected = Cat10::same(*view, self.view);
                                if nav_item(ui, pal, pal.accent, *icon, label, selected).clicked() {
                                    self.go(*view);
                                }
                            }
                        }
                        None => {
                            for cat in Cat10::ALL {
                                if nav_item(ui, pal, pal.accent, cat.icon(), cat.title(), false)
                                    .clicked()
                                {
                                    self.go(cat.items()[0].2);
                                }
                            }
                        }
                    }
                    ui.add_space(8.0);
                });
        });

        // Account strip along the bottom of the pane.
        let strip = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - account_h), rect.max);
        ui.painter().rect_filled(
            Rect::from_min_size(strip.min, Vec2::new(strip.width(), 1.0)),
            0,
            pal.card_border,
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(strip.shrink2(Vec2::new(12.0, 8.0)))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                avatar(ui, pal, &self.info.full_name, 32.0);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        egui::RichText::new(&self.info.full_name)
                            .font(semibold(13.0))
                            .color(pal.text),
                    );
                    ui.label(
                        egui::RichText::new(if self.info.is_admin {
                            "Administrator"
                        } else {
                            "Standard user"
                        })
                        .font(regular(11.0))
                        .color(pal.text_faint),
                    );
                });
            },
        );
    }

    pub(crate) fn content(&mut self, ui: &mut Ui, pal: &Palette, rect: Rect) {
        let view = self.view;
        let searching = !self.search.trim().is_empty();
        if !searching && matches!(view, View::Home) && pal.design == crate::theme::Design::Win10 {
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                pages::home10(self, ui, rect, pal);
            });
            return;
        }
        if !searching && matches!(view, View::Page(Page::Apps)) && !self.hub.tried {
            self.hub_refresh();
        }
        // Check for updates once per run, in the background, so the counts
        // on Home, System, Apps › Updates and Software updates are real.
        static AUTO_CHECKED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if self.updates_checked.is_none()
            && !self.updates_loading
            && std::env::var("WIN11_SETTINGS_SCREENSHOT").is_err()
            && !AUTO_CHECKED.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            self.check_updates();
        }
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            ScrollArea::vertical()
                .id_salt(("content", if searching { "search" } else { view.salt() }))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(rect.width());
                    egui::Frame::new()
                        .inner_margin(if pal.design == crate::theme::Design::Win10 {
                            Margin {
                                left: 30,
                                right: 24,
                                top: 6,
                                bottom: 30,
                            }
                        } else {
                            Margin {
                                left: 34,
                                right: 26,
                                top: 8,
                                bottom: 32,
                            }
                        })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if searching {
                                pages::search(self, ui);
                                return;
                            }
                            match view {
                                View::Home => {
                                    if pal.design == crate::theme::Design::Win10 {
                                        pages::home10(self, ui, rect, &pal)
                                    } else {
                                        pages::system(self, ui)
                                    }
                                }
                                View::Page(Page::System) => pages::system(self, ui),
                                View::Page(Page::Bluetooth) => pages::bluetooth(self, ui),
                                View::Page(Page::Network) => pages::network(self, ui),
                                View::Page(Page::Personalisation) => {
                                    pages::personalisation(self, ui)
                                }
                                View::Page(Page::Desktop) => crate::desktop::desktop_page(self, ui),
                                View::Page(Page::Extensions) => {
                                    crate::desktop::extensions_page(self, ui)
                                }
                                View::Page(Page::Apps) => pages::apps(self, ui),
                                View::Page(Page::Accounts) => pages::accounts(self, ui),
                                View::Page(Page::Accessibility) => pages::accessibility(self, ui),
                                View::Page(Page::Privacy) => pages::privacy(self, ui),
                                View::Page(Page::TimeLanguage) => pages::time_language(self, ui),
                                View::Page(Page::Compatibility) => pages::compatibility(self, ui),
                                View::Page(Page::Updates) => pages::updates(self, ui),
                                View::Page(Page::Advanced) => crate::advanced::advanced(self, ui),
                                View::Detail(_, Detail::Display) => pages::display(self, ui),
                                View::Detail(_, Detail::Sound) => pages::sound(self, ui),
                                View::Detail(_, Detail::Notifications) => {
                                    pages::notifications(self, ui)
                                }
                                View::Detail(_, Detail::Power) => pages::power(self, ui),
                                View::Detail(_, Detail::Storage) => pages::storage(self, ui),
                                View::Detail(_, Detail::About) => pages::about(self, ui),
                                View::Detail(_, Detail::Pointer) => pages::pointer(self, ui),
                                View::Detail(_, Detail::Keyboard) => pages::keyboard(self, ui),
                                View::Detail(_, Detail::DefaultApps) => {
                                    pages::default_apps(self, ui)
                                }
                                View::Detail(_, Detail::AdvancedNetwork) => {
                                    crate::advanced::advanced_network(self, ui)
                                }
                                View::Detail(_, Detail::Desk(i)) => {
                                    crate::desktop::desktop_section(self, ui, i)
                                }
                            }
                        });
                });
        });
    }

    pub(crate) fn toast(&mut self, ctx: &Context, pal: &Palette) {
        let Some(toast) = &self.toast else { return };
        if Instant::now() >= toast.until {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        let fill = if toast.error {
            Color32::from_rgb(0x5A, 0x1F, 0x1F)
        } else {
            pal.tooltip
        };
        // Light fills (the light theme's tooltip colour) need dark text.
        let ink = if u32::from(fill.r()) + u32::from(fill.g()) + u32::from(fill.b()) > 3 * 150 {
            pal.text
        } else {
            Color32::WHITE
        };
        egui::Area::new(Id::new("toast"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -22.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(16, 9))
                    .show(ui, |ui| {
                        // One line up to 520 px, wrapping only past that.
                        let w = ui
                            .painter()
                            .layout_no_wrap(toast.text.clone(), regular(13.0), ink)
                            .size()
                            .x;
                        ui.set_width(w.min(520.0));
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&toast.text)
                                    .font(regular(13.0))
                                    .color(ink),
                            )
                            .wrap(),
                        );
                    });
            });
    }

    pub(crate) fn maybe_screenshot(&mut self, ctx: &Context) {
        let Some(path) = self.screenshot_path.clone() else {
            return;
        };
        let waiting_hub = matches!(self.view, View::Page(Page::Apps)) && self.hub.loading;
        let waiting_updates = self.updates_rx.is_some();
        let waiting_updates = waiting_updates || self.photo_rx.is_some();
        // Testing hook: capture the very first frame, before the live probe
        // returns, to prove cached values are already visible.
        let early = std::env::var("WIN11_SETTINGS_SHOT_EARLY").is_ok();
        if !self.screenshot_requested
            && (early
                || (self.info_rx.is_none()
                    && !waiting_hub
                    && !waiting_updates
                    && !self.cleaner.scanning
                    && !crate::advanced::LOADING.load(std::sync::atomic::Ordering::Relaxed)))
        {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
            self.screenshot_requested = true;
            ctx.request_repaint_after(Duration::from_millis(120));
            return;
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let [w, h] = image.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for pixel in &image.pixels {
                rgba.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b(), pixel.a()]);
            }
            match image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
                Ok(()) => eprintln!("Screenshot written to {path}"),
                Err(err) => eprintln!("Could not write screenshot: {err}"),
            }
            self.screenshot_path = None;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }

    pub(crate) fn handle_resize(&self, ctx: &Context) {
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !ctx.input(|i| i.pointer.primary_pressed()) {
            return;
        }
        // A maximized or fullscreen window cannot be resized from its edges.
        let (maximized, fullscreen) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.maximized.unwrap_or(false),
                vp.fullscreen.unwrap_or(false),
            )
        });
        if maximized || fullscreen {
            return;
        }
        // The shadow margin around the window body is a resize handle too.
        let body = ctx.viewport_rect().shrink(FRAME_MARGIN);
        let direction = resize_direction(pos, body, title_height(self.design));
        if let Some(direction) = direction {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}

/// Which window edge a press at `pos` should resize, if any. The caption
/// buttons reach the window edge, so their outer margin must click the
/// button, not start a resize (Close at the very corner, Windows 11 style).
pub(crate) fn resize_direction(
    pos: Pos2,
    screen: Rect,
    title_h: f32,
) -> Option<egui::ResizeDirection> {
    let captions = Rect::from_min_max(
        Pos2::new(screen.right() - 46.0 * 3.0, screen.top()),
        Pos2::new(screen.right(), screen.top() + title_h),
    );
    if captions.contains(pos) {
        return None;
    }
    let edge = 6.0;
    let near_left = pos.x - screen.left() <= edge;
    let near_right = screen.right() - pos.x <= edge;
    let near_top = pos.y - screen.top() <= edge;
    let near_bottom = screen.bottom() - pos.y <= edge;
    match (near_top, near_bottom, near_left, near_right) {
        (true, _, true, _) => Some(egui::ResizeDirection::NorthWest),
        (true, _, _, true) => Some(egui::ResizeDirection::NorthEast),
        (_, true, true, _) => Some(egui::ResizeDirection::SouthWest),
        (_, true, _, true) => Some(egui::ResizeDirection::SouthEast),
        (true, _, _, _) => Some(egui::ResizeDirection::North),
        (_, true, _, _) => Some(egui::ResizeDirection::South),
        (_, _, true, _) => Some(egui::ResizeDirection::West),
        (_, _, _, true) => Some(egui::ResizeDirection::East),
        _ => None,
    }
}

#[cfg(test)]
mod resize_tests {
    use super::*;
    use egui::ResizeDirection as D;

    pub(crate) fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))
    }

    #[test]
    pub(crate) fn close_button_margin_is_not_a_resize_edge() {
        for pos in [
            (1279.0, 0.0),
            (1279.0, 20.0),
            (1250.0, 0.0),
            (1145.0, 2.0),
            (1279.0, 47.0),
        ] {
            assert_eq!(
                resize_direction(Pos2::new(pos.0, pos.1), screen(), TITLE_H),
                None,
                "{pos:?}"
            );
        }
    }

    #[test]
    pub(crate) fn edges_outside_captions_still_resize() {
        assert_eq!(
            resize_direction(Pos2::new(1279.0, 100.0), screen(), TITLE_H),
            Some(D::East)
        );
        assert_eq!(
            resize_direction(Pos2::new(500.0, 1.0), screen(), TITLE_H),
            Some(D::North)
        );
        assert_eq!(
            resize_direction(Pos2::new(1.0, 1.0), screen(), TITLE_H),
            Some(D::NorthWest)
        );
        assert_eq!(
            resize_direction(Pos2::new(1279.0, 799.0), screen(), TITLE_H),
            Some(D::SouthEast)
        );
        assert_eq!(
            resize_direction(Pos2::new(600.0, 400.0), screen(), TITLE_H),
            None
        );
    }
}
