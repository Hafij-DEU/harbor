//! The application shell: custom title bar, sidebar, navigation and overlays.

mod actions;
mod apps_actions;
mod background;
mod chrome;
mod dialogs;
mod nav;
mod state;

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Context, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use crate::system::{self, Info};
use crate::theme::{self, Mode, Palette, ACCENTS};
use crate::widgets;
use actions::*;
use chrome::*;
pub use nav::*;
pub use state::*;

pub const TITLE_H: f32 = 48.0;

/// Windows 10 title bars are half the height of Windows 11 ones.
pub const TITLE_H_10: f32 = 32.0;

/// Clear margin around the window body for its shadow (not maximized).
pub const FRAME_MARGIN: f32 = 14.0;

/// Corner radius of the window body, as Harbor Files (GTK) windows.
const FRAME_RADIUS: u8 = 12;

pub const SIDEBAR_W: f32 = 300.0;

/// Keep Settings comfortably readable even when Ubuntu's text scale is 100%.
pub const APP_ZOOM: f32 = 1.15;

pub struct App {
    pub info: Info,
    info_rx: Option<Receiver<Info>>,

    pub view: View,
    pub search: String,
    pub focus_search: bool,

    pub mode: Mode,
    pub accent: Color32,
    pub follow_system: bool,
    /// Windows 11 (default) or the Windows 10 interface.
    pub design: crate::theme::Design,
    /// Windows 10 home: list each category's pages on its card.
    pub home_links: bool,
    /// Accent or Colourful icons (Personalisation › Colour style).
    pub look: crate::theme::Look,
    /// Recently accessed pages, newest first (home and System page tiles).
    pub recent: Vec<View>,
    /// Glass values per scope (`system::GlassScope::ALL` order); `None`
    /// follows the scope above.
    pub glass_scopes: [Option<crate::theme::Glass>; 5],
    /// The scope Personalisation › Glass effects is editing.
    pub glass_edit: system::GlassScope,
    /// The window was created see-through (glass opacity below 100).
    pub glass_window: bool,
    /// Keyboard Manager page state (see `pages/keyboard/state.rs`).
    pub keyboard: crate::pages::KeyboardUi,
    /// Corner radius of the window body this frame (0 when maximized).
    pub frame_r: u8,
    /// Pointer position, for "Light follows pointer".
    pub pointer: Option<Pos2>,
    /// A scope changed while a slider was dragged; saved on release.
    pub glass_dirty: Option<system::GlassScope>,
    last_mode: Mode,
    last_accent: Color32,
    last_design: crate::theme::Design,
    appearance_rx: Receiver<system::Appearance>,
    pub system_mode: Mode,
    pub system_accent: Option<Color32>,
    pub system_accent_name: String,
    pub a11y: system::A11y,
    pub privacy: system::Privacy,
    icons: std::collections::HashMap<String, Option<egui::TextureHandle>>,

    toast: Option<Toast>,
    pub rename: Option<RenameDialog>,
    rename_rx: Option<Receiver<Result<(), String>>>,
    rename_target: String,
    pub wifi_connect: Option<WifiDialog>,
    wallpaper_rx: Option<Receiver<Result<String, String>>>,
    /// The account picture, drawn in every avatar.
    photo: Option<egui::TextureHandle>,
    /// Loading or changing the account picture: Ok(None) means no picture.
    photo_rx: Option<Receiver<Result<Option<(usize, Vec<u8>)>, String>>>,
    auto_updates_rx: Option<Receiver<Result<bool, String>>>,
    /// Background `pkexec timedatectl set-timezone` in flight.
    timezone_rx: Option<Receiver<Result<String, String>>>,
    /// Background `bluetoothctl scan on` in flight.
    bt_scan_rx: Option<Receiver<Result<(), String>>>,
    pub bt_scanning: bool,
    /// Filter text of the Keyboard page "add layout" list.
    pub keyboard_filter: String,
    /// Filter text for the Time zone list.
    pub timezone_filter: String,

    /// Walking the home directory in the background (can take seconds).
    home_usage_rx: Option<Receiver<Option<f32>>>,
    /// Any settings write in flight, plus its follow-up.
    sys_action_rx: Option<Receiver<(SysAction, Result<(), String>)>>,
    /// Modal for the account name / password.
    pub text_prompt: Option<TextPrompt>,

    pub clock_refresh: Instant,
    pub updates_loading: bool,
    updates_rx: Option<Receiver<Vec<system::PackageUpdate>>>,
    pub updates_checked: Option<Instant>,

    pub test_volume: f32,
    pub test_brightness: f32,
    pub test_mic_volume: f32,
    pub text_scale: f32,

    pub hub: HubState,
    pub apt_job: Option<AptJob>,
    /// The last update job, shown with its log until the user closes it.
    pub apt_done: Option<crate::jobview::DoneJob>,
    /// The last App Hub / uninstall job, shown on the Apps page.
    pub hub_done: Option<crate::jobview::DoneJob>,
    pub cleaner: CleanerState,
    pub confirm: Option<ConfirmDialog>,
    pub activity: Vec<ActivityEntry>,
    pub activity_log_open: bool,

    pub welcome: Option<crate::welcome::Welcome>,
    pub desk: crate::desktop::State,
    screenshot_path: Option<String>,
    screenshot_requested: bool,
    first_frame_logged: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);

        let a11y = system::accessibility();
        let privacy = system::privacy();
        let scale = a11y.text_scale;
        // Testing hook: shrink the whole UI so a full page fits in a screenshot.
        let zoom = std::env::var("WIN11_SETTINGS_ZOOM")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(scale * APP_ZOOM);
        if (0.5..=2.0).contains(&zoom) {
            cc.egui_ctx.set_zoom_factor(zoom);
        }

        let appearance = system::appearance();
        let system_accent = appearance
            .accent
            .map(|(r, g, b)| Color32::from_rgb(r, g, b));
        let follow_system = match std::env::var("WIN11_SETTINGS_MODE").as_deref() {
            Ok("dark") => false,
            Ok("light") => false,
            Ok("custom") => false,
            _ => true,
        };
        let mode = match std::env::var("WIN11_SETTINGS_MODE").as_deref() {
            Ok("dark") => Mode::Dark,
            Ok("light") => Mode::Light,
            _ if follow_system => appearance.mode,
            _ => Mode::Dark,
        };
        let accent = system_accent.unwrap_or(ACCENTS[1].1);

        let (appearance_tx, appearance_rx) = mpsc::channel();
        {
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || loop {
                let snapshot = system::appearance();
                if appearance_tx.send(snapshot).is_err() {
                    break;
                }
                ctx.request_repaint();
                std::thread::sleep(Duration::from_millis(2500));
            });
        }

        let design = system::saved_design().unwrap_or(crate::theme::Design::Win11);
        let pal = Palette::new(mode, accent, design);
        theme::apply_style(&cc.egui_ctx, &pal, mode);

        // Show the previous snapshot straight away so storage and hardware
        // values are on screen in the first frame; the live probe replaces
        // them a moment later.
        let startup_info = system::cached_info();
        if std::env::var("WIN11_SETTINGS_STARTUP").is_ok() {
            eprintln!(
                "startup snapshot: {} at {:.0} ms",
                if startup_info.is_some() {
                    "hit"
                } else {
                    "miss"
                },
                crate::START
                    .get()
                    .map(|t| t.elapsed().as_secs_f64() * 1000.0)
                    .unwrap_or(0.0)
            );
        }

        let (tx, rx) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            // Testing hook: hold the live probe back so the first paint can be
            // inspected while old values are still on screen.
            if std::env::var("WIN11_SETTINGS_SLOW_PROBE").is_ok() {
                std::thread::sleep(Duration::from_secs(4));
            }
            let info = Info::load();
            let _ = tx.send(info);
            ctx.request_repaint();
        });

        let requested_view = std::env::var("WIN11_SETTINGS_VIEW").unwrap_or_default();
        let view = match requested_view.as_str() {
            "home" if design == crate::theme::Design::Win10 => View::Home,
            "home" => View::Page(Page::System),
            "system" => View::Page(Page::System),
            "display" => View::Detail(Page::System, Detail::Display),
            "sound" => View::Detail(Page::System, Detail::Sound),
            "notifications" => View::Detail(Page::System, Detail::Notifications),
            "power" => View::Detail(Page::System, Detail::Power),
            "storage" => View::Detail(Page::System, Detail::Storage),
            "about" => View::Detail(Page::System, Detail::About),
            "pointer" => View::Detail(Page::System, Detail::Pointer),
            "keyboard" => View::Detail(Page::System, Detail::Keyboard),
            "defaultapps" => View::Detail(Page::System, Detail::DefaultApps),
            "advanced" => View::Page(Page::Advanced),
            key if key.starts_with("tweaks-") => TWEAK_GROUPS
                .iter()
                .position(|g| g.3 == key)
                .map_or(View::Page(Page::Advanced), |i| {
                    View::Detail(Page::Advanced, Detail::TweakGroup(i as u8))
                }),
            "dns" | "advanced-network" => View::Detail(Page::Network, Detail::AdvancedNetwork),
            "bluetooth" => View::Page(Page::Bluetooth),
            "network" => View::Page(Page::Network),
            "personalisation" => View::Page(Page::Personalisation),
            "apps" => View::Page(Page::Apps),
            // Kept for old launch links: App Hub is now part of Apps.
            "apphub" => View::Page(Page::Apps),
            "accounts" => View::Page(Page::Accounts),
            "desktop" | "tweaks" => View::Page(Page::Desktop),
            "multitasking" => View::Detail(Page::Desktop, Detail::Desk(0)),
            "dock" => View::Detail(Page::Desktop, Detail::Desk(1)),
            "startup" => View::Detail(Page::Desktop, Detail::Desk(7)),
            "fusion" => View::Detail(Page::Desktop, Detail::Desk(8)),
            "fusion-taskbar" => View::Detail(Page::Desktop, Detail::Desk(11)),
            "extensions" => View::Page(Page::Extensions),
            "accessibility" => View::Page(Page::Accessibility),
            "privacy" => View::Page(Page::Privacy),
            "time" => View::Page(Page::TimeLanguage),
            "compatibility" => View::Page(Page::Compatibility),
            "updates" => View::Page(Page::Updates),
            _ if design == crate::theme::Design::Win10 => View::Home,
            _ => View::Page(Page::System),
        };

        let mut app = Self {
            info: startup_info.unwrap_or_else(Info::placeholder),
            info_rx: Some(rx),
            view,
            search: String::new(),
            focus_search: false,
            mode,
            accent,
            follow_system,
            design,
            home_links: system::saved_home_links(),
            look: system::saved_look(),
            recent: system::saved_recent(),
            glass_scopes: system::GlassScope::ALL.map(system::scope_glass),
            glass_edit: system::GlassScope::Harbor,
            glass_window: system::glass_window(),
            frame_r: 0,
            keyboard: Default::default(),
            pointer: None,
            glass_dirty: None,
            last_mode: mode,
            last_accent: accent,
            last_design: design,
            appearance_rx,
            system_mode: appearance.mode,
            system_accent,
            system_accent_name: appearance.accent_name.clone(),
            a11y,
            privacy,
            icons: std::collections::HashMap::new(),
            toast: None,
            rename: None,
            rename_rx: None,
            rename_target: String::new(),
            wifi_connect: None,
            wallpaper_rx: None,
            photo: None,
            photo_rx: None,
            auto_updates_rx: None,
            timezone_rx: None,
            bt_scan_rx: None,
            bt_scanning: false,
            keyboard_filter: String::new(),
            timezone_filter: String::new(),
            home_usage_rx: None,
            sys_action_rx: None,
            text_prompt: None,
            clock_refresh: Instant::now(),
            updates_loading: false,
            updates_rx: None,
            updates_checked: None,
            test_volume: 0.5,
            test_brightness: 1.0,
            test_mic_volume: 0.7,
            text_scale: scale.clamp(1.0, 1.75),
            hub: HubState::default(),
            apt_job: None,
            apt_done: None,
            hub_done: None,
            cleaner: CleanerState::default(),
            confirm: None,
            activity: Vec::new(),
            activity_log_open: false,
            welcome: None,
            desk: Default::default(),
            screenshot_path: std::env::var("WIN11_SETTINGS_SCREENSHOT").ok(),
            screenshot_requested: false,
            first_frame_logged: false,
        };
        if crate::hub::available() {
            app.hub.tab = AppsTab::MyApps;
        }
        match std::env::var("WIN11_SETTINGS_APPS_TAB").as_deref() {
            Ok("myapps") => app.hub.tab = AppsTab::MyApps,
            Ok("updates") => app.hub.tab = AppsTab::Updates,
            Ok("library") => app.hub.tab = AppsTab::Library,
            Ok("allapps") => app.hub.tab = AppsTab::AllApps,
            _ => {}
        }
        if crate::welcome::show_at_start() {
            app.welcome = Some(crate::welcome::Welcome::new(&app));
        }
        app.start_home_usage();
        app.load_photo(None);
        if std::env::var("WIN11_SETTINGS_SCREENSHOT").is_err() {
            app.hub_refresh();
            app.check_updates();
        } else if let Ok(kind) = std::env::var("WIN11_SETTINGS_FAKE_JOB") {
            app.fake_job(&kind);
        } else if std::env::var("WIN11_SETTINGS_OPEN_CLEANER").is_ok() {
            // Screenshot of Free Up Space (`browsers` = only the browser groups).
            app.open_cleaner();
        }
        app
    }

    /// Load (and cache) the themed or absolute-path icon for an application.
    /// Missing icons return `None`, and callers fall back to a painted tile.
    pub fn app_icon(&mut self, ctx: &Context, name: &str) -> Option<egui::TextureHandle> {
        if name.is_empty() {
            return None;
        }
        if let Some(hit) = self.icons.get(name) {
            return hit.clone();
        }
        let path = if name.starts_with('/') {
            let candidate = std::path::PathBuf::from(name);
            candidate.is_file().then_some(candidate)
        } else {
            system::find_icon(name)
        };
        let loaded = path.and_then(|path| image::open(path).ok()).map(|img| {
            let img = img.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let pixels = img.into_raw();
            let color = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            ctx.load_texture(
                format!("app-icon:{name}"),
                color,
                egui::TextureOptions::LINEAR,
            )
        });
        self.icons.insert(name.to_owned(), loaded.clone());
        loaded
    }

    pub fn pal(&self) -> Palette {
        let glass = self.glass();
        // A window created opaque cannot turn see-through until reopened.
        let see = self.glass_window && glass.see_through();
        let mut pal = Palette::new(self.mode, self.accent, self.design).with_glass(glass, see);
        pal.pointer = self.pointer;
        pal.look = self.look;
        pal
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        self.note_recent();
        if std::env::var("WIN11_SETTINGS_STARTUP").is_ok() && !self.first_frame_logged {
            self.first_frame_logged = true;
            eprintln!(
                "first frame at {:.0} ms",
                crate::START
                    .get()
                    .map(|t| t.elapsed().as_secs_f64() * 1000.0)
                    .unwrap_or(0.0)
            );
        }
        let ctx = root.ctx().clone();
        self.poll_background(&ctx);
        self.handle_shortcuts(&ctx);

        if self.mode != self.last_mode
            || self.accent != self.last_accent
            || self.design != self.last_design
        {
            let pal = Palette::new(self.mode, self.accent, self.design);
            theme::apply_style(&ctx, &pal, self.mode);
            self.last_mode = self.mode;
            self.last_accent = self.accent;
            self.last_design = self.design;
        }

        self.pointer = if self.glass().light_follows {
            ctx.input(|i| i.pointer.hover_pos())
        } else {
            None
        };
        let pal = self.pal();
        self.handle_resize(&ctx);

        // Like GTK windows (Harbor Files): rounded corners and a soft shadow
        // in a clear margin; maximized and full-screen windows fill the screen.
        let (maximized, fullscreen) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.maximized.unwrap_or(false),
                vp.fullscreen.unwrap_or(false),
            )
        });
        let framed = !(maximized || fullscreen);
        let outer = root.max_rect();
        let full = if framed {
            outer.shrink(FRAME_MARGIN)
        } else {
            outer
        };
        self.frame_r = if framed { FRAME_RADIUS } else { 0 };
        if framed {
            // GNOME's window shadow: soft and a little below the window,
            // lighter when not focused. egui's feathered shadow has no hard
            // outline (rings of 1 px strokes left a line at the edge).
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
            let (alpha, blur, drop) = if focused { (70, 20, 3) } else { (40, 14, 2) };
            let shadow = egui::epaint::Shadow {
                offset: [0, drop],
                blur,
                spread: 0,
                color: Color32::from_black_alpha(alpha),
            };
            root.painter().add(shadow.as_shape(full, self.frame_r));
        }
        root.painter().rect_filled(full, self.frame_r, pal.bg);
        widgets::glow_backdrop(root.painter(), full, &pal);

        let title_h = title_height(self.design);
        let title_rect = Rect::from_min_size(full.min, Vec2::new(full.width(), title_h));
        self.titlebar(root, &ctx, &pal, title_rect);

        let body = Rect::from_min_max(Pos2::new(full.left(), full.top() + title_h), full.max);
        // The Windows 10 home screen uses the full window width; every other
        // view keeps its navigation pane.
        let home = pal.design == crate::theme::Design::Win10
            && matches!(self.view, View::Home)
            && self.search.trim().is_empty();
        if home {
            self.content(root, &pal, body);
        } else {
            let sidebar_w = if pal.design == crate::theme::Design::Win10 {
                SIDEBAR_W.min(body.width() * 0.32).max(220.0)
            } else {
                SIDEBAR_W.min(body.width() * 0.36).max(240.0)
            };
            let side_rect = Rect::from_min_size(body.min, Vec2::new(sidebar_w, body.height()));
            let content_rect =
                Rect::from_min_max(Pos2::new(side_rect.right(), body.top()), body.max);

            self.sidebar(root, &pal, side_rect);
            self.content(root, &pal, content_rect);
        }

        if framed {
            root.painter().rect_stroke(
                full,
                FRAME_RADIUS,
                Stroke::new(1.0, pal.divider),
                StrokeKind::Inside,
            );
        }

        self.dialogs(&ctx, &pal);
        self.toast(&ctx, &pal);
        self.maybe_screenshot(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // The window is always clear: the body fill sets how much of the
        // desktop shows through, and the margin holds the shadow. Clear to
        // black, since the compositor reads premultiplied alpha and any
        // colour here would be added over the desktop as a light haze.
        [0.0, 0.0, 0.0, 0.0]
    }
}
