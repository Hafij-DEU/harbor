//! The application shell: custom title bar, sidebar, navigation and overlays.

use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, Context, CursorIcon, Id, Layout, Margin, Pos2, Rect,
    ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, ViewportCommand,
};

use crate::cleaner;
use crate::hub::{self, Action as HubAction, HubApp};
use crate::inventory;
use crate::icons::Icon;
use crate::pages;
use crate::system::{self, Info};
use crate::theme::{self, regular, semibold, Mode, Palette, ACCENTS};
use crate::widgets::{self, avatar, caption_button, nav_item, Caption};

pub const TITLE_H: f32 = 48.0;
pub const SIDEBAR_W: f32 = 300.0;
/// Keep Settings comfortably readable even when Ubuntu's text scale is 100%.
pub const APP_ZOOM: f32 = 1.15;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    System,
    Bluetooth,
    Network,
    Personalisation,
    Desktop,
    Extensions,
    Apps,
    Accounts,
    Accessibility,
    Privacy,
    TimeLanguage,
    Compatibility,
    Updates,
}

impl Page {
    pub const ALL: [Page; 13] = [
        Page::System,
        Page::Bluetooth,
        Page::Network,
        Page::Personalisation,
        Page::Desktop,
        Page::Extensions,
        Page::Apps,
        Page::Accounts,
        Page::Accessibility,
        Page::Privacy,
        Page::TimeLanguage,
        Page::Compatibility,
        Page::Updates,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::System => "System",
            Page::Bluetooth => "Bluetooth & devices",
            Page::Network => "Network & internet",
            Page::Personalisation => "Personalisation",
            Page::Desktop => "Desktop & windows",
            Page::Extensions => "Extensions",
            Page::Apps => "Apps",
            Page::Accounts => "Accounts",
            Page::Accessibility => "Accessibility",
            Page::Privacy => "Privacy & security",
            Page::TimeLanguage => "Time & language",
            Page::Compatibility => "Compatibility",
            Page::Updates => "Software updates",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Page::System => Icon::System,
            Page::Bluetooth => Icon::Bluetooth,
            Page::Network => Icon::Wifi,
            Page::Personalisation => Icon::Personalise,
            Page::Desktop => Icon::Display,
            Page::Extensions => Icon::Hub,
            Page::Apps => Icon::Apps,
            Page::Accounts => Icon::Accounts,
            Page::Accessibility => Icon::Accessibility,
            Page::Privacy => Icon::Shield,
            Page::TimeLanguage => Icon::Clock,
            Page::Compatibility => Icon::Compatibility,
            Page::Updates => Icon::Update,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    Display,
    Sound,
    Notifications,
    Power,
    Storage,
    About,
    Pointer,
    Keyboard,
    DefaultApps,
}

impl Detail {
    pub fn title(self) -> &'static str {
        match self {
            Detail::Display => "Display",
            Detail::Sound => "Sound",
            Detail::Notifications => "Notifications",
            Detail::Power => "Power & battery",
            Detail::Storage => "Storage",
            Detail::About => "About",
            Detail::Pointer => "Mouse & touchpad",
            Detail::Keyboard => "Keyboard",
            Detail::DefaultApps => "Default applications",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Page(Page),
    Detail(Page, Detail),
}

impl View {
    pub fn page(self) -> Page {
        match self {
            View::Page(p) => p,
            View::Detail(p, _) => p,
        }
    }

    fn salt(self) -> &'static str {
        match self {
            View::Page(Page::System) => "system",
            View::Page(Page::Bluetooth) => "bluetooth",
            View::Page(Page::Network) => "network",
            View::Page(Page::Personalisation) => "personalisation",
            View::Page(Page::Desktop) => "desktop",
            View::Page(Page::Extensions) => "extensions",
            View::Page(Page::Apps) => "apps",
            View::Page(Page::Accounts) => "accounts",
            View::Page(Page::Accessibility) => "accessibility",
            View::Page(Page::Privacy) => "privacy",
            View::Page(Page::TimeLanguage) => "time",
            View::Page(Page::Compatibility) => "compat",
            View::Page(Page::Updates) => "updates",
            View::Detail(_, Detail::Display) => "display",
            View::Detail(_, Detail::Sound) => "sound",
            View::Detail(_, Detail::Notifications) => "notifications",
            View::Detail(_, Detail::Power) => "power",
            View::Detail(_, Detail::Storage) => "storage",
            View::Detail(_, Detail::About) => "about",
            View::Detail(_, Detail::Pointer) => "pointer",
            View::Detail(_, Detail::Keyboard) => "keyboard",
            View::Detail(_, Detail::DefaultApps) => "default-apps",
        }
    }
}

pub struct RenameDialog {
    pub buf: String,
    pub error: Option<String>,
    pub busy: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    RealName,
    Password,
    ProxyHttp,
    ProxySocks,
    ProxyIgnore,
}


pub struct TextPrompt {
    pub kind: PromptKind,
    pub value: String,
    pub busy: bool,
    pub error: Option<String>,
}

/// A settings change running off the UI thread, with what to do once it lands.
pub enum SysAction {
    RealName(String),
    Password,
    Autologin(bool),
    Locale(String),
    Display {
        connector: String,
        mode_id: String,
        scale: f32,
    },
    Ntp(bool),
    WifiRadio(bool),
    Discoverable(bool),
    Proxy,
}

pub struct WifiDialog {
    pub ssid: String,
    pub secured: bool,
    pub password: String,
    pub error: Option<String>,
}

pub struct AptJob {
    pub label: String,
    pub log: Vec<String>,
    rx: Receiver<crate::jobs::JobEvent>,
}

#[derive(Default)]
pub struct CleanerState {
    pub open: bool,
    pub scanning: bool,
    pub groups: Vec<cleaner::CleanerGroup>,
    pub error: Option<String>,
    rx: Option<Receiver<Result<Vec<cleaner::CleanerGroup>, String>>>,
}

pub enum ConfirmKind {
    InstallUpdates,
    Uninstall { id: String, name: String },
    RemoveSysApp { app: inventory::SysApp },
    ForgetWifi,
    InstallCompat { package: String, name: String },
    InstallFavorite { key: String },
    RemoveExtension { uuid: String },
}

pub struct ConfirmDialog {
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub kind: ConfirmKind,
    pub busy: bool,
}

pub struct HubJob {
    pub label: String,
    pub log: Vec<String>,
    rx: Receiver<crate::jobs::JobEvent>,
}

#[derive(Clone)]
pub struct ActivityEntry {
    pub title: String,
    pub details: String,
    pub error: bool,
}

/// Which tab of the Apps page is open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppsTab {
    MyApps,
    Updates,
    Library,
    AllApps,
}

#[derive(Default)]
pub struct HubState {
    pub installed: bool,
    pub version: String,
    pub apps: Vec<HubApp>,
    /// The Library (extra GitHub projects with .deb releases), App Hub only.
    pub library: Vec<hub::LibraryApp>,
    /// Saved install recipes from App Hub's Favorites.
    pub favorites: Vec<hub::Favorite>,
    pub library_error: Option<String>,
    /// Every installed desktop app on the system (All Apps inventory).
    pub sys_apps: Vec<inventory::SysApp>,
    pub sys_loading: bool,
    pub sys_error: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub filter: String,
    pub tried: bool,
    /// Which Apps-page tab is open.
    pub tab: AppsTab,
    /// Ids that belong in My Apps (user's catalogue + user-added), refreshed with the data.
    pub my_ids: std::collections::HashSet<String>,
    /// Live text of the Library "add app" box.
    pub library_input: String,
    rx: Option<
        Receiver<(
            String,
            Result<Vec<HubApp>, String>,
            Result<Vec<hub::LibraryApp>, String>,
            Vec<hub::Favorite>,
        )>,
    >,
    sys_rx: Option<Receiver<Result<Vec<inventory::SysApp>, String>>>,
    pub job: Option<HubJob>,
}

impl Default for AppsTab {
    fn default() -> Self {
        AppsTab::AllApps
    }
}

struct Toast {
    text: String,
    until: Instant,
    error: bool,
}

pub struct App {
    pub info: Info,
    info_rx: Option<Receiver<Info>>,

    pub view: View,
    pub search: String,
    pub focus_search: bool,

    pub mode: Mode,
    pub accent: Color32,
    pub follow_system: bool,
    last_mode: Mode,
    last_accent: Color32,
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

        let pal = Palette::new(mode, accent);
        theme::apply_style(&cc.egui_ctx, &pal, mode);

        // Show the previous snapshot straight away so storage and hardware
        // values are on screen in the first frame; the live probe replaces
        // them a moment later.
        let startup_info = system::cached_info();
        if std::env::var("WIN11_SETTINGS_STARTUP").is_ok() {
            eprintln!(
                "startup snapshot: {} at {:.0} ms",
                if startup_info.is_some() { "hit" } else { "miss" },
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

        let view = match std::env::var("WIN11_SETTINGS_VIEW")
            .unwrap_or_default()
            .as_str()
        {
            "display" => View::Detail(Page::System, Detail::Display),
            "sound" => View::Detail(Page::System, Detail::Sound),
            "notifications" => View::Detail(Page::System, Detail::Notifications),
            "power" => View::Detail(Page::System, Detail::Power),
            "storage" => View::Detail(Page::System, Detail::Storage),
            "about" => View::Detail(Page::System, Detail::About),
            "pointer" => View::Detail(Page::System, Detail::Pointer),
            "keyboard" => View::Detail(Page::System, Detail::Keyboard),
            "defaultapps" => View::Detail(Page::System, Detail::DefaultApps),
            "bluetooth" => View::Page(Page::Bluetooth),
            "network" => View::Page(Page::Network),
            "personalisation" => View::Page(Page::Personalisation),
            "apps" => View::Page(Page::Apps),
            // Kept for old launch links: App Hub is now part of Apps.
            "apphub" => View::Page(Page::Apps),
            "accounts" => View::Page(Page::Accounts),
            "desktop" | "tweaks" | "multitasking" => View::Page(Page::Desktop),
            "extensions" => View::Page(Page::Extensions),
            "accessibility" => View::Page(Page::Accessibility),
            "privacy" => View::Page(Page::Privacy),
            "time" => View::Page(Page::TimeLanguage),
            "compatibility" => View::Page(Page::Compatibility),
            "updates" => View::Page(Page::Updates),
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
            last_mode: mode,
            last_accent: accent,
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
        if std::env::var("WIN11_SETTINGS_SCREENSHOT").is_err() {
            app.hub_refresh();
            app.check_updates();
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
        let loaded = path
            .and_then(|path| image::open(path).ok())
            .map(|img| {
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
        Palette::new(self.mode, self.accent)
    }

    /// Refresh the system probe in the background (polled in `poll_background`).
    pub fn reload_info(&mut self) {
        if self.info_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        // Keep showing the current values while the refresh runs, instead of
        // blanking the whole app back to placeholders.
        self.info_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(Info::load());
        });
    }

    /// Start the background home-directory size walk (the one probe that can
    /// take seconds on a large home folder).
    pub fn start_home_usage(&mut self) {
        if self.home_usage_rx.is_some() || self.info.home_used_gb.is_some() {
            return;
        }
        // At startup `info` is still a placeholder, so fall back to $HOME.
        let home = if self.info.home.is_empty() {
            std::env::var("HOME").unwrap_or_default()
        } else {
            self.info.home.clone()
        };
        if home.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.home_usage_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(system::home_usage_gb(&home));
        });
    }

    /// Run a settings write in the background and apply the result when it
    /// returns. Keeps every authentication/network call off the UI thread.
    pub fn run_sys(
        &mut self,
        action: SysAction,
        work: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) {
        if self.sys_action_rx.is_some() {
            self.notify("Another change is still saving…");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.sys_action_rx = Some(rx);
        std::thread::spawn(move || {
            let result = work();
            let _ = tx.send((action, result));
        });
    }

    pub fn apply_display(&mut self, connector: String, mode_id: String, scale: f32) {
        let c = connector.clone();
        let m = mode_id.clone();
        self.run_sys(
            SysAction::Display {
                connector,
                mode_id,
                scale,
            },
            move || system::set_display_mode(&c, &m, scale),
        );
    }

    pub fn set_autologin(&mut self, on: bool) {
        let user = self.info.user.clone();
        self.run_sys(SysAction::Autologin(on), move || {
            system::set_autologin(on, &user)
        });
    }

    pub fn set_ntp(&mut self, on: bool) {
        self.run_sys(SysAction::Ntp(on), move || system::set_ntp(on));
    }

    pub fn set_wifi_radio(&mut self, on: bool) {
        self.run_sys(SysAction::WifiRadio(on), move || system::set_wifi_enabled(on));
    }

    pub fn set_discoverable(&mut self, on: bool) {
        self.run_sys(SysAction::Discoverable(on), move || {
            system::set_discoverable(on)
        });
    }

    pub fn set_locale(&mut self, locale: String) {
        let l = locale.clone();
        self.run_sys(SysAction::Locale(locale), move || system::set_locale(&l));
    }

    pub fn open_rename_account(&mut self) {
        self.text_prompt = Some(TextPrompt {
            kind: PromptKind::RealName,
            value: self.info.full_name.clone(),
            busy: false,
            error: None,
        });
    }

    pub fn open_change_password(&mut self) {
        self.text_prompt = Some(TextPrompt {
            kind: PromptKind::Password,
            value: String::new(),
            busy: false,
            error: None,
        });
    }

    pub fn open_proxy_prompt(&mut self, kind: PromptKind) {
        let value = match kind {
            PromptKind::ProxyHttp => format!(
                "{}:{}",
                self.info.proxy_http_host, self.info.proxy_http_port
            ),
            PromptKind::ProxySocks => format!(
                "{}:{}",
                self.info.proxy_socks_host, self.info.proxy_socks_port
            ),
            _ => self.info.proxy_ignore.clone(),
        };
        self.text_prompt = Some(TextPrompt {
            kind,
            value,
            busy: false,
            error: None,
        });
    }

    /// Write the proxy state currently held in `info` to the desktop settings.
    pub fn apply_proxy(&mut self) {
        let mode = self.info.proxy_mode.clone();
        let http_host = self.info.proxy_http_host.clone();
        let http_port = self.info.proxy_http_port;
        let socks_host = self.info.proxy_socks_host.clone();
        let socks_port = self.info.proxy_socks_port;
        let ignore = self.info.proxy_ignore.clone();
        self.run_sys(SysAction::Proxy, move || {
            if system::set_proxy(
                &mode,
                &http_host,
                http_port,
                &socks_host,
                socks_port,
                &ignore,
            ) {
                Ok(())
            } else {
                Err("Could not save the proxy settings".to_owned())
            }
        });
    }

    pub fn set_proxy_mode(&mut self, mode: &str) {
        self.info.proxy_mode = mode.to_owned();
        self.apply_proxy();
    }

    pub fn ensure_sys_apps(&mut self) {
        if self.hub.sys_rx.is_some() || !self.hub.sys_apps.is_empty() {
            return;
        }
        if self.hub.sys_error.is_some() {
            return;
        }
        self.hub.sys_loading = true;
        let (tx, rx) = mpsc::channel();
        self.hub.sys_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(inventory::list());
        });
    }

    /// Refresh the App Hub catalogue, Library and the standalone installed-apps
    /// inventory in the background (network work happens inside `app-hub list`,
    /// which is what App Hub itself does).
    pub fn hub_refresh(&mut self) {
        if self.hub.loading {
            return;
        }
        self.hub.loading = true;
        self.hub.library_error = None;
        self.hub.tried = true;
        self.hub.error = None;
        let (tx, rx) = mpsc::channel();
        self.hub.rx = Some(rx);
        std::thread::spawn(move || {
            let version = hub::version();
            let result = if hub::available() {
                hub::list()
            } else {
                Err("The app-hub command was not found. Install App Hub first.".to_owned())
            };
            let library = hub::load_library();
            let favorites = hub::load_favorites();
            let _ = tx.send((version, result, library, favorites));
        });
    }

    /// Confirm installing a favorite saved in App Hub.
    pub fn open_install_favorite(&mut self, key: &str) {
        let Some(favorite) = self.hub.favorites.iter().find(|f| f.key() == key) else {
            return;
        };
        if !favorite.installable() {
            self.notify_error("Install this one from App Hub — it downloads a file");
            return;
        }
        let name = if favorite.name.is_empty() {
            favorite.id.clone()
        } else {
            favorite.name.clone()
        };
        let body = format!(
            "App Hub saved this install:\n\n{}\n\nThis needs your administrator password{}.",
            favorite.describe(),
            if favorite.channel == "hub" {
                " (through App Hub)"
            } else {
                ""
            }
        );
        self.confirm = Some(ConfirmDialog {
            title: format!("Install {name}?"),
            body,
            confirm_label: "Install".to_owned(),
            kind: ConfirmKind::InstallFavorite {
                key: favorite.key(),
            },
            busy: false,
        });
    }

    pub fn run_favorite_install(&mut self, key: &str) {
        if self.hub.job.is_some() {
            return;
        }
        let Some(favorite) = self.hub.favorites.iter().find(|f| f.key() == key).cloned() else {
            return;
        };
        let label = format!("Installing {}", favorite.name);
        self.record_activity(label.clone(), favorite.describe(), false);
        self.hub.job = Some(HubJob {
            label,
            log: Vec::new(),
            rx: hub::run_favorite(&favorite),
        });
    }

    /// Start an install/update/open job. Output is reported once the job ends.
    pub fn hub_act(&mut self, action: hub::Action, id: &str) {
        if self.hub.job.is_some() {
            return;
        }
        let label = format!("{} {}", action.verb(), pretty_id(id));
        self.record_activity(label.clone(), "Started.", false);
        self.hub.job = Some(HubJob {
            label,
            log: Vec::new(),
            rx: hub::run_action(action, id),
        });
    }

    pub fn use_system_appearance(&mut self) {
        self.follow_system = true;
        self.mode = self.system_mode;
        if let Some(accent) = self.system_accent {
            self.accent = accent;
        }
    }

    pub fn apply_appearance_to_desktop(&mut self) {
        let mode_ok = system::set_color_scheme(self.mode);
        let accent_ok = system::set_accent(self.accent.r(), self.accent.g(), self.accent.b());
        if mode_ok && accent_ok {
            self.notify("Ubuntu appearance updated");
        } else {
            self.notify_error("Could not write the Ubuntu appearance settings");
        }
    }

    pub fn open_install_updates(&mut self) {
        self.confirm = Some(ConfirmDialog {
            title: "Install updates".to_owned(),
            body: "Ubuntu will download and install every pending package update. \
This needs your administrator password and can take a few minutes."
                .to_owned(),
            confirm_label: "Install now".to_owned(),
            kind: ConfirmKind::InstallUpdates,
            busy: false,
        });
    }

    pub fn open_uninstall(&mut self, id: &str, name: &str) {
        self.confirm = Some(ConfirmDialog {
            title: format!("Uninstall {name}?"),
            body: "The application and its system package will be removed. \
Your personal files are kept."
                .to_owned(),
            confirm_label: "Uninstall".to_owned(),
            kind: ConfirmKind::Uninstall {
                id: id.to_owned(),
                name: name.to_owned(),
            },
            busy: false,
        });
    }

    /// Confirm before uninstalling a system app (the “All installed
    /// applications” inventory), showing what apt would also remove.
    pub fn open_remove_sys_app(&mut self, app: &inventory::SysApp) {
        let mut lines = Vec::new();
        let channel = match app.channel.as_str() {
            "apt" => "Deb package",
            "snap" => "Snap package",
            "flatpak" => "Flatpak application",
            "local" => "Local application",
            _ => "Application",
        };
        lines.push(format!("{channel} · {}", app.id));
        if !app.version.is_empty() {
            lines.push(format!("Version {}", app.version));
        }
        if !app.source.is_empty() {
            lines.push(format!("Installed from {}", app.source));
        }
        let size = app.size_label();
        if !size.is_empty() {
            lines.push(format!("Size {size}"));
        }
        lines.push(String::new());
        match app.channel.as_str() {
            "apt" => lines.push(
                "Uninstalled with apt-get remove — one password prompt. Your personal files are kept."
                    .to_owned(),
            ),
            "snap" => lines.push(
                "Uninstalled with snap remove — one password prompt. Your personal files are kept."
                    .to_owned(),
            ),
            "flatpak" if app.installation == "user" => lines.push(
                "Removed with flatpak — no password prompt needed.".to_owned(),
            ),
            "flatpak" => lines.push(
                "Removed from the system Flatpak installation — one password prompt. Your personal files are kept."
                    .to_owned(),
            ),
            "local" => lines.push(
                "The launcher and the program it runs are moved to the Trash — recoverable."
                    .to_owned(),
            ),
            _ => lines.push("This application cannot be uninstalled from here.".to_owned()),
        }

        let (preview, risky) = if app.channel == "apt" {
            let (all, risky_list) = inventory::apt_removal_preview(&app.id);
            (
                all.into_iter().filter(|p| p != &app.id).collect::<Vec<_>>(),
                risky_list,
            )
        } else {
            (Vec::new(), Vec::new())
        };
            if !preview.is_empty() {
                lines.push(String::new());
                let shown = preview.len().min(8);
                let mut text = format!(
                    "Also removes {} package{}: {}",
                    preview.len(),
                    if preview.len() == 1 { "" } else { "s" },
                    preview[..shown].join(", ")
                );
                if preview.len() > shown {
                    text.push_str(&format!(" and {} more", preview.len() - shown));
                }
                lines.push(text);
                if !risky.is_empty() {
                    lines.push(String::new());
                    lines.push(format!(
                        "Warning: this includes parts of the desktop ({}). Only uninstall if you are sure.",
                        risky.join(", ")
                    ));
                }
            }

        self.confirm = Some(ConfirmDialog {
            title: format!("Uninstall {}?", app.name),
            body: lines.join("\n"),
            confirm_label: "Uninstall".to_owned(),
            kind: ConfirmKind::RemoveSysApp { app: app.clone() },
            busy: false,
        });
    }

    pub fn run_install_updates(&mut self) {
        if self.apt_job.is_some() {
            return;
        }
        let label = "Installing Ubuntu updates".to_owned();
        self.record_activity(label.clone(), "Started.", false);
        self.apt_job = Some(AptJob {
            label,
            log: Vec::new(),
            rx: system::install_updates(),
        });
    }

    pub fn run_refresh_lists(&mut self) {
        if self.apt_job.is_some() {
            return;
        }
        let label = "Refreshing package lists".to_owned();
        self.record_activity(label.clone(), "Started.", false);
        self.apt_job = Some(AptJob {
            label,
            log: Vec::new(),
            rx: system::refresh_package_lists(),
        });
    }

    /// Open the Free Up Space dialog and kick off a background scan.
    pub fn open_cleaner(&mut self) {
        self.cleaner.open = true;
        if self.cleaner.scanning || !self.cleaner.groups.is_empty() {
            return;
        }
        self.cleaner_scan_start();
    }

    pub fn cleaner_scan_start(&mut self) {
        if self.cleaner.scanning {
            return;
        }
        self.cleaner.scanning = true;
        self.cleaner.error = None;
        let (tx, rx) = mpsc::channel();
        self.cleaner.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(cleaner::scan());
        });
    }

    /// Carry out the selected clean-up. Output streams into the activity log.
    pub fn cleaner_clean(&mut self) {
        let sel = cleaner::selection(&self.cleaner.groups);
        let empty = sel.trash.is_empty() && sel.plan.is_empty() && sel.user.is_empty();
        if empty {
            return;
        }
        let bytes = sel.selected_bytes;
        let label = format!("Freeing up space ({})", cleaner::human_bytes(bytes));
        self.record_activity(label.clone(), "Started.", false);
        self.hub.job = Some(HubJob {
            label,
            log: Vec::new(),
            rx: cleaner::execute(&sel),
        });
        self.cleaner.open = false;
        self.cleaner.groups.clear(); // rescan next time so sizes are fresh
    }

    pub fn hub_update_all(&mut self) {
        if self.hub.job.is_some() {
            return;
        }
        let label = "Updating all managed applications".to_owned();
        self.record_activity(label.clone(), "Started.", false);
        self.hub.job = Some(HubJob {
            label,
            log: Vec::new(),
            rx: hub::update_all(),
        });
    }

    /// Add a GitHub project to the Library (`app-hub add <spec>`). The App Hub
    /// job stream lands in the Activity Log; a refresh follows when it ends.
    pub fn library_add(&mut self) {
        let spec = self.hub.library_input.trim().to_owned();
        if spec.is_empty() {
            return;
        }
        if self.hub.job.is_some() {
            self.notify_error("Wait for the current job to finish first");
            return;
        }
        self.hub.library_input.clear();
        let label = format!("Adding {spec} to the Library");
        self.record_activity(label.clone(), "Started.", false);
        self.hub.job = Some(HubJob {
            label,
            log: Vec::new(),
            rx: hub::add_library_app(&spec),
        });
    }

    /// Remove an entry from the Library (or from My Apps) — it does not
    /// uninstall anything. Quick local bookkeeping inside App Hub.
    pub fn library_remove(&mut self, id: &str, name: &str) {
        let script = format!(
            "import sys; sys.path.insert(0, \"/usr/lib/app-hub\"); \\
             from app_hub import library; library.remove({id})",
            id = serde_json::to_string(id).unwrap_or_else(|_| "\"\"".to_owned())
        );
        let result = std::process::Command::new("python3")
            .args(["-Is", "-c", &script])
            .stdin(std::process::Stdio::null())
            .output();
        match result {
            Ok(out) if out.status.success() => {
                self.notify(format!("{name} removed from the Library"));
                self.hub_refresh();
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
                self.notify_error(if err.is_empty() {
                    format!("Could not remove {name} from the Library")
                } else {
                    err
                });
            }
            Err(err) => self.notify_error(format!("Could not run App Hub: {err}")),
        }
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast {
            text: msg.into(),
            until: Instant::now() + Duration::from_secs(3),
            error: false,
        });
    }

    pub fn notify_error(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast {
            text: msg.into(),
            until: Instant::now() + Duration::from_secs(5),
            error: true,
        });
    }

    pub fn record_activity(
        &mut self,
        title: impl Into<String>,
        details: impl Into<String>,
        error: bool,
    ) {
        self.activity.push(ActivityEntry {
            title: title.into(),
            details: details.into(),
            error,
        });
        if self.activity.len() > 300 {
            self.activity.remove(0);
        }
    }

    /// Drain pending output lines from a background job into its log buffer.
    /// Returns `Some((exit_code, full_log))` once the job has finished.
    fn drain_job(
        log: &mut Vec<String>,
        rx: &Receiver<crate::jobs::JobEvent>,
    ) -> Option<(i32, String)> {
        loop {
            match rx.try_recv() {
                Ok(crate::jobs::JobEvent::Line(line)) => {
                    if !line.trim().is_empty() {
                        log.push(line);
                        if log.len() > 400 {
                            log.remove(0);
                        }
                    }
                }
                Ok(crate::jobs::JobEvent::Finished { code, log: full }) => {
                    let log = if full.trim().is_empty() {
                        std::mem::take(log).join("\n")
                    } else {
                        full
                    };
                    return Some((code, log));
                }
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let joined = std::mem::take(log).join("\n");
                    return Some((-1, joined));
                }
            }
        }
    }

    /// Shared completion handling for hub and apt jobs: toast, activity log
    /// entry, and a refresh of the data the job changed.
    fn finish_job(&mut self, label: &str, code: i32, log: String, success: bool, hub_job: bool) {
        if hub_job {
            if success {
                self.notify(format!("{label} finished"));
                self.record_activity(
                    format!("{label} completed"),
                    if log.trim().is_empty() {
                        "Completed successfully.".to_owned()
                    } else {
                        log
                    },
                    false,
                );
            } else {
                self.notify_error(format!("{label} failed — see the activity log"));
                self.record_activity(format!("{label} failed"), log, true);
            }
            self.hub.job = None;
            self.hub.tried = false;
            self.hub_refresh();
        } else if success {
            self.record_activity(
                format!("{label} completed"),
                if log.trim().is_empty() {
                    "Completed successfully.".to_owned()
                } else {
                    log
                },
                false,
            );
            self.apt_job = None;
        } else {
            self.notify_error(format!("{label} failed — see the activity log"));
            let _ = code;
            self.record_activity(format!("{label} failed"), log, true);
            self.apt_job = None;
        }
    }

    pub fn open_rename(&mut self) {
        self.rename = Some(RenameDialog {
            buf: self.info.hostname.clone(),
            error: None,
            busy: false,
        });
        self.rename_target = self.info.hostname.clone();
    }

    pub fn open_wifi_connect(&mut self, ssid: &str, secured: bool) {
        self.wifi_connect = Some(WifiDialog {
            ssid: ssid.to_owned(),
            secured,
            password: String::new(),
            error: None,
        });
    }

    /// Pick a wallpaper with a native file chooser (zenity) and apply it to
    /// the light, dark and lock screen backgrounds.
    pub fn open_wallpaper_picker(&mut self) {
        if self.wallpaper_rx.is_some() {
            return;
        }
        if !system::command_exists("zenity") {
            self.notify_error("zenity is not installed");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.wallpaper_rx = Some(rx);
        std::thread::spawn(move || {
            let out = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--title=Choose a wallpaper",
                    "--file-filter=Images | *.png *.jpg *.jpeg *.webp *.bmp *.heic",
                ])
                .output();
            match out {
                Ok(o) if o.status.success() => {
                    let path = String::from_utf8_lossy(&o.stdout).trim().to_owned();
                    let _ = tx.send(if path.is_empty() {
                        Err("No file was selected.".to_owned())
                    } else {
                        Ok(path)
                    });
                }
                Ok(_) => {
                    let _ = tx.send(Err("Selection cancelled.".to_owned()));
                }
                Err(err) => {
                    let _ = tx.send(Err(format!("Could not run zenity: {err}")));
                }
            }
        });
    }

    fn apply_wallpaper(&mut self, path: &str) {
        if system::set_wallpaper(path) {
            self.info.wallpaper = format!("file://{path}");
            self.notify("Wallpaper applied");
            self.record_activity("Wallpaper changed", path.to_owned(), false);
        } else {
            self.notify_error("Could not set the wallpaper");
        }
    }

    /// Toggle unattended security upgrades (pkexec writes the apt config).
    pub fn set_automatic_updates(&mut self, on: bool) {
        if self.auto_updates_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.auto_updates_rx = Some(rx);
        std::thread::spawn(move || {
            let ok = system::set_unattended_upgrades(on);
            let _ = tx.send(if ok {
                Ok(on)
            } else {
                Err("Could not change automatic updates (cancelled or not authorised).".to_owned())
            });
        });
    }

    /// Change the system time zone through PolicyKit (background thread).
    pub fn set_timezone(&mut self, tz: String) {
        if self.timezone_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.timezone_rx = Some(rx);
        std::thread::spawn(move || {
            let ok = system::set_timezone(&tz);
            let _ = tx.send(if ok {
                Ok(tz)
            } else {
                Err("Could not change the time zone (cancelled or not authorised).".to_owned())
            });
        });
    }

    /// Ask for confirmation before forgetting the active Wi-Fi network.
    pub fn open_forget_wifi(&mut self) {
        self.confirm = Some(ConfirmDialog {
            title: "Forget this Wi-Fi network?".to_owned(),
            body: "NetworkManager will delete the saved connection, including its password. \
The network will not reconnect automatically."
                .to_owned(),
            confirm_label: "Forget network".to_owned(),
            kind: ConfirmKind::ForgetWifi,
            busy: false,
        });
    }

    /// Ask before removing a GNOME Shell extension the user installed.
    pub fn confirm_extension_removal(&mut self, uuid: String, name: String) {
        self.confirm = Some(ConfirmDialog {
            title: format!("Remove {name}?"),
            body: "The extension is turned off and its files are deleted. You can install it again later."
                .to_owned(),
            confirm_label: "Remove".to_owned(),
            kind: ConfirmKind::RemoveExtension { uuid },
            busy: false,
        });
    }

    /// Install a compatibility package (Wine/Waydroid/Steam) through pkexec apt.
    pub fn run_compat_install(&mut self, package: &str, name: &str) {
        if self.apt_job.is_some() {
            return;
        }
        let label = format!("Installing {name}");
        self.record_activity(label.clone(), "Started.", false);
        self.apt_job = Some(AptJob {
            label,
            log: Vec::new(),
            rx: system::install_package(package),
        });
    }

    pub fn check_updates(&mut self) {
        if self.updates_loading {
            return;
        }
        self.updates_loading = true;
        let (tx, rx) = mpsc::channel();
        self.updates_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(system::check_updates());
        });
    }

    /// Start a Bluetooth discovery pass without freezing the UI: the button
    /// used to call bluetoothctl synchronously and the window stopped
    /// responding until the scan timed out.
    pub fn start_bluetooth_scan(&mut self, ctx: &Context) {
        if self.bt_scan_rx.is_some() {
            return;
        }
        self.bt_scanning = true;
        let (tx, rx) = mpsc::channel();
        self.bt_scan_rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = system::scan_bluetooth();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    pub fn go(&mut self, view: View) {
        self.view = view;
        self.search.clear();
    }

    pub fn back(&mut self) {
        match self.view {
            View::Detail(page, _) => self.view = View::Page(page),
            View::Page(_) => {}
        }
    }

    /// Apply the visible effect of a completed settings write.
    fn finish_sys(&mut self, action: SysAction) {
        match action {
            SysAction::RealName(name) => {
                self.info.full_name = name;
                self.text_prompt = None;
                self.notify("Account name updated");
            }
            SysAction::Password => {
                self.text_prompt = None;
                self.notify("Password updated");
            }
            SysAction::Autologin(on) => {
                self.info.autologin = Some(on);
                self.notify(if on {
                    "Automatic sign-in turned on"
                } else {
                    "Automatic sign-in turned off"
                });
            }
            SysAction::Locale(locale) => {
                self.info.locale = locale;
                self.notify("Language set — sign out and back in to apply it");
            }
            SysAction::Display {
                connector,
                mode_id,
                scale,
            } => {
                for monitor in &mut self.info.display_monitors {
                    if monitor.connector != connector {
                        continue;
                    }
                    monitor.scale = scale;
                    for mode in &mut monitor.modes {
                        mode.current = mode.id == mode_id;
                        if mode.current {
                            self.info.resolution = format!("{} × {}", mode.width, mode.height);
                            self.info.refresh = format!("{:.0} Hz", mode.rate);
                        }
                    }
                }
                self.notify("Display updated");
            }
            SysAction::Ntp(on) => {
                self.info.ntp_enabled = on;
                self.notify(if on {
                    "Automatic time turned on"
                } else {
                    "Automatic time turned off"
                });
            }
            SysAction::WifiRadio(on) => {
                self.info.wifi_enabled = on;
                self.notify(if on { "Wi-Fi turned on" } else { "Wi-Fi turned off" });
                self.reload_info();
            }
            SysAction::Proxy => {
                self.notify("Proxy settings saved");
            }
            SysAction::Discoverable(on) => {
                self.info.discoverable = on;
                self.notify(if on {
                    "This device is now discoverable"
                } else {
                    "This device is no longer discoverable"
                });
            }
        }
    }

    fn poll_background(&mut self, ctx: &Context) {
        while let Ok(snapshot) = self.appearance_rx.try_recv() {
            self.system_mode = snapshot.mode;
            self.system_accent_name = snapshot.accent_name;
            if let Some((r, g, b)) = snapshot.accent {
                self.system_accent = Some(Color32::from_rgb(r, g, b));
            }
            if self.follow_system {
                self.mode = snapshot.mode;
                if let Some(accent) = self.system_accent {
                    self.accent = accent;
                }
            }
        }

        if self.cleaner.rx.is_some() {
            let received = self.cleaner.rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(groups))) => {
                    self.cleaner.scanning = false;
                    self.cleaner.rx = None;
                    self.cleaner.groups = groups;
                }
                Some(Ok(Err(err))) => {
                    self.cleaner.scanning = false;
                    self.cleaner.rx = None;
                    self.cleaner.error = Some(err);
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.cleaner.scanning = false;
                    self.cleaner.rx = None;
                    self.cleaner.error = Some("The cleaner stopped unexpectedly.".to_owned());
                }
                None => {}
            }
        }

        if self.timezone_rx.is_some() {
            let received = self.timezone_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(tz))) => {
                    self.timezone_rx = None;
                    self.info.timezone = tz.clone();
                    self.notify(format!("Time zone changed to {tz}"));
                    self.reload_info();
                }
                Some(Ok(Err(err))) => {
                    self.timezone_rx = None;
                    self.notify_error(err);
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(250));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.timezone_rx = None;
                    self.notify_error("The time zone change stopped unexpectedly");
                }
                None => {}
            }
        }

        if self.home_usage_rx.is_some() {
            let received = self.home_usage_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(used)) => {
                    self.info.home_used_gb = used;
                    self.home_usage_rx = None;
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(400));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.home_usage_rx = None;
                }
                None => {}
            }
        }

        if self.bt_scan_rx.is_some() {
            let received = self.bt_scan_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(()))) => {
                    self.bt_scanning = false;
                    self.bt_scan_rx = None;
                    self.notify("Bluetooth scan complete");
                    self.reload_info();
                }
                Some(Ok(Err(err))) => {
                    self.bt_scanning = false;
                    self.bt_scan_rx = None;
                    self.notify_error(if err.is_empty() {
                        "Could not scan for Bluetooth devices".to_owned()
                    } else {
                        err
                    });
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.bt_scanning = false;
                    self.bt_scan_rx = None;
                    self.notify_error("The Bluetooth scan stopped unexpectedly");
                }
                None => {}
            }
        }

        if self.sys_action_rx.is_some() {
            let received = self.sys_action_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok((action, result))) => {
                    self.sys_action_rx = None;
                    match result {
                        Ok(()) => self.finish_sys(action),
                        Err(err) => {
                            let inline = matches!(
                                action,
                                SysAction::RealName(_) | SysAction::Password
                            );
                            if inline {
                                if let Some(prompt) = &mut self.text_prompt {
                                    prompt.busy = false;
                                    prompt.error = Some(err.clone());
                                }
                                self.notify_error(&err);
                            } else {
                                self.notify_error(&err);
                            }
                        }
                    }
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.sys_action_rx = None;
                }
                None => {}
            }
        }

        if self.info_rx.is_some() {
            let received = self.info_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(info)) => {
                    self.test_volume = info.volume as f32 / 100.0;
                    self.test_brightness = info.brightness.unwrap_or(100) as f32 / 100.0;
                    self.test_mic_volume = info.microphone_volume as f32 / 100.0;
                    // The system probe does not know about package updates, so keep
                    // whatever the apt check has already found.
                    let updates = self.info.updates.take();
                    let home_used = self.info.home_used_gb;
                    self.info = info;
                    self.info.updates = updates;
                    if self.info.home_used_gb.is_none() {
                        self.info.home_used_gb = home_used;
                    }
                    self.info_rx = None;
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(120));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.info_rx = None;
                }
                None => {}
            }
        }

        if self.updates_rx.is_some() {
            let received = self.updates_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(packages)) => {
                    self.info.updates = Some(packages);
                    self.updates_loading = false;
                    self.updates_rx = None;
                    self.updates_checked = Some(Instant::now());
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.updates_loading = false;
                    self.updates_rx = None;
                }
                None => {}
            }
        }

        if self.hub.rx.is_some() {
            let received = self.hub.rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok((version, result, library, favorites))) => {
                    self.hub.loading = false;
                    self.hub.rx = None;
                    self.hub.installed = !version.is_empty() || result.is_ok();
                    self.hub.version = version;
                    match result {
                        Ok(apps) => {
                            self.hub.apps = apps;
                            self.hub.error = None;
                        }
                        Err(err) => {
                            self.hub.error = Some(err);
                        }
                    }
                    match library {
                        Ok(apps) => {
                            self.hub.my_ids = hub::my_app_ids(&apps);
                            self.hub.library = apps;
                            self.hub.library_error = None;
                        }
                        Err(err) => {
                            self.hub.library = Vec::new();
                            self.hub.library_error = Some(err);
                            self.hub.my_ids = hub::my_app_ids(&[]);
                        }
                    }
                    self.hub.favorites = favorites;
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.hub.loading = false;
                    self.hub.rx = None;
                    self.hub.error = Some("App Hub stopped unexpectedly.".to_owned());
                }
                None => {}
            }
        }

        if self.hub.sys_rx.is_some() {
            let received = self.hub.sys_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(sys)) => {
                    self.hub.sys_loading = false;
                    self.hub.sys_rx = None;
                    match sys {
                        Ok(apps) => {
                            self.hub.sys_apps = apps;
                            self.hub.sys_error = None;
                        }
                        Err(err) => {
                            self.hub.sys_error = Some(err);
                        }
                    }
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.hub.sys_loading = false;
                    self.hub.sys_rx = None;
                    self.hub.sys_error = Some("The installed-apps scan stopped unexpectedly.".to_owned());
                }
                None => {}
            }
        }

        if let Some(job) = self.hub.job.as_mut() {
            let label = job.label.clone();
            match Self::drain_job(&mut job.log, &job.rx) {
                None => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
                Some((code, log)) => {
                    let success = code == 0 || log.contains("Everything is up to date");
                    self.finish_job(&label, code, log, success, true);
                }
            }
        }

        if let Some(job) = self.apt_job.as_mut() {
            let label = job.label.clone();
            match Self::drain_job(&mut job.log, &job.rx) {
                None => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
                Some((code, log)) => {
                    let success = code == 0;
                    let installing = label.contains("Installing");
                    let nothing = log.contains("0 upgraded, 0 newly installed");
                    self.finish_job(&label, code, log, success, false);
                    if installing && success {
                        self.info.updates = None;
                        self.check_updates();
                        self.notify(if nothing {
                            "No packages needed installing".to_owned()
                        } else if label == "Installing Ubuntu updates" {
                            "Ubuntu updates installed".to_owned()
                        } else {
                            format!("{label} completed")
                        });
                    } else if label.contains("Refreshing") && success {
                        self.notify("Package lists refreshed");
                    }
                }
            }
        }

        if self.rename_rx.is_some() {
            let received = self.rename_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(()))) => {
                    self.info.hostname = self.rename_target.clone();
                    self.rename = None;
                    self.rename_rx = None;
                    self.notify("Device name updated");
                }
                Some(Ok(Err(err))) => {
                    if let Some(dialog) = &mut self.rename {
                        dialog.busy = false;
                        dialog.error = Some(err);
                    }
                    self.rename_rx = None;
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.rename_rx = None;
                }
                None => {}
            }
        }

        if self.timezone_rx.is_some() {
            let received = self.timezone_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(tz))) => {
                    self.timezone_rx = None;
                    self.notify("Time zone updated");
                    self.reload_info();
                    self.info.timezone = tz;
                    self.timezone_filter.clear();
                }
                Some(Ok(Err(err))) => {
                    self.timezone_rx = None;
                    self.notify_error(&err);
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.timezone_rx = None;
                }
                None => {}
            }
        }

        if self.wallpaper_rx.is_some() {
            let received = self.wallpaper_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(path))) => {
                    self.wallpaper_rx = None;
                    self.apply_wallpaper(&path);
                }
                Some(Ok(Err(err))) => {
                    if !err.contains("cancelled") {
                        self.notify_error(&err);
                    }
                    self.wallpaper_rx = None;
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.wallpaper_rx = None;
                }
                None => {}
            }
        }

        if self.auto_updates_rx.is_some() {
            let received = self.auto_updates_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(Ok(on))) => {
                    self.auto_updates_rx = None;
                    self.info.automatic_updates = Some(on);
                    self.notify(if on {
                        "Automatic security updates on"
                    } else {
                        "Automatic security updates off"
                    });
                }
                Some(Ok(Err(err))) => {
                    self.auto_updates_rx = None;
                    self.notify_error(&err);
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.auto_updates_rx = None;
                }
                None => {}
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        // The welcome guide handles its own keys (Esc closes it).
        if self.welcome.is_some() {
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

    fn titlebar(&mut self, ui: &mut Ui, ctx: &Context, pal: &Palette, rect: Rect) {
        let buttons_w = 46.0 * 3.0;
        let drag_rect =
            Rect::from_min_max(rect.min, Pos2::new(rect.right() - buttons_w, rect.bottom()));
        let response = ui.interact(drag_rect, Id::new("titlebar-drag"), Sense::click_and_drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if response.double_clicked() {
            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }

        let dot = Pos2::new(rect.left() + 18.0, rect.center().y);
        ui.painter().circle_filled(dot, 2.6, pal.accent);
        ui.painter().text(
            Pos2::new(rect.left() + 28.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Settings",
            regular(12.5),
            pal.text_muted,
        );

        let max = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        // Caption buttons fill the title bar and touch the window edge, so the
        // very corner of the screen still hits Close (Windows 11 behaviour).
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
                if caption_button(ui, pal, Caption::Minimize, pal.accent).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                }
                let glyph = if max {
                    Caption::Restore
                } else {
                    Caption::Maximize
                };
                if caption_button(ui, pal, glyph, pal.accent).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
                }
                if caption_button(ui, pal, Caption::Close, pal.accent).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            },
        );
    }

    fn sidebar(&mut self, ui: &mut Ui, pal: &Palette, rect: Rect) {
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

    fn content(&mut self, ui: &mut Ui, _pal: &Palette, rect: Rect) {
        let view = self.view;
        let searching = !self.search.trim().is_empty();
        if !searching && matches!(view, View::Page(Page::Apps)) && !self.hub.tried {
            self.hub_refresh();
        }
        if !searching
            && matches!(view, View::Page(Page::Updates))
            && self.updates_checked.is_none()
            && !self.updates_loading
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
                        .inner_margin(Margin {
                            left: 34,
                            right: 26,
                            top: 8,
                            bottom: 32,
                        })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if searching {
                                pages::search(self, ui);
                                return;
                            }
                            match view {
                                View::Page(Page::System) => pages::system(self, ui),
                                View::Page(Page::Bluetooth) => pages::bluetooth(self, ui),
                                View::Page(Page::Network) => pages::network(self, ui),
                                View::Page(Page::Personalisation) => {
                                    pages::personalisation(self, ui)
                                }
                                View::Page(Page::Desktop) => crate::desktop::desktop_page(self, ui),
                                View::Page(Page::Extensions) => crate::desktop::extensions_page(self, ui),
                                View::Page(Page::Apps) => pages::apps(self, ui),
                                View::Page(Page::Accounts) => pages::accounts(self, ui),
                                View::Page(Page::Accessibility) => pages::accessibility(self, ui),
                                View::Page(Page::Privacy) => pages::privacy(self, ui),
                                View::Page(Page::TimeLanguage) => pages::time_language(self, ui),
                                View::Page(Page::Compatibility) => pages::compatibility(self, ui),
                                View::Page(Page::Updates) => pages::updates(self, ui),
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
                            }
                        });
                });
        });
    }

    fn text_prompt_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(prompt) = self.text_prompt.as_mut() else {
            return;
        };
        let (title, label, hint) = match prompt.kind {
            PromptKind::RealName => (
                "Change your account name",
                "Name",
                "Shown on the sign-in screen and in the top bar.",
            ),
            PromptKind::Password => (
                "Change your password",
                "New password",
                "You will be asked for your administrator password to confirm.",
            ),
            PromptKind::ProxyHttp => (
                "HTTP and HTTPS proxy",
                "Server address",
                "Use the form host:port, for example proxy.example.com:8080.",
            ),
            PromptKind::ProxySocks => (
                "SOCKS proxy",
                "Server address",
                "Use the form host:port, for example 127.0.0.1:1080.",
            ),
            PromptKind::ProxyIgnore => (
                "Ignored hosts",
                "Hosts and domains",
                "Separate entries with commas, for example localhost, 127.0.0.0/8.",
            ),
        };
        let mut close = false;
        let mut save = false;

        egui::Window::new(title)
            .id(Id::new("text-prompt-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    egui::RichText::new(title)
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(hint)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(label)
                        .font(regular(12.0))
                        .color(pal.text_faint),
                );
                ui.add_space(4.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut prompt.value)
                        .password(prompt.kind == PromptKind::Password)
                        .font(regular(14.0))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(8, 6)),
                );
                if !prompt.busy && ctx.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                if let Some(err) = &prompt.error {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(err)
                            .font(regular(12.0))
                            .color(Color32::from_rgb(0xFF, 0x99, 0x99)),
                    );
                }
                if prompt.busy {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Saving…")
                                .font(regular(12.0))
                                .color(pal.text_faint),
                        );
                    });
                }
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, pal, pal.accent, "Save", None).clicked()
                        && !prompt.busy
                    {
                        save = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() && !prompt.busy {
                        close = true;
                    }
                });
            });

        let mut submit: Option<(PromptKind, String)> = None;
        if save && !prompt.busy {
            let value = prompt.value.trim().to_owned();
            prompt.error = None;
            match prompt.kind {
                PromptKind::RealName if value.is_empty() => {
                    prompt.error = Some("Enter your name.".to_owned());
                }
                PromptKind::Password if value.len() < 4 => {
                    prompt.error = Some("Use at least 4 characters.".to_owned());
                }
                PromptKind::ProxyHttp | PromptKind::ProxySocks
                    if parse_host_port(&value).0.is_empty() =>
                {
                    prompt.error = Some("Enter a server address.".to_owned());
                }
                kind => {
                    prompt.busy = true;
                    submit = Some((kind, value));
                }
            }
        }
        if close && !prompt.busy {
            self.text_prompt = None;
        }
        if let Some((kind, value)) = submit {
            match kind {
                PromptKind::RealName => self.run_sys(SysAction::RealName(value.clone()), move || {
                    system::set_real_name(&value)
                }),
                PromptKind::Password => {
                    self.run_sys(SysAction::Password, move || system::set_password(&value))
                }
                PromptKind::ProxyHttp => {
                    let (host, port) = parse_host_port(&value);
                    self.info.proxy_http_host = host;
                    self.info.proxy_http_port = port;
                    self.info.proxy_mode = "manual".to_owned();
                    self.apply_proxy();
                }
                PromptKind::ProxySocks => {
                    let (host, port) = parse_host_port(&value);
                    self.info.proxy_socks_host = host;
                    self.info.proxy_socks_port = port;
                    self.info.proxy_mode = "manual".to_owned();
                    self.apply_proxy();
                }
                PromptKind::ProxyIgnore => {
                    self.info.proxy_ignore = value;
                    self.apply_proxy();
                }
            }
        }
    }

    fn rename_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(dialog) = self.rename.as_mut() else {
            return;
        };
        let mut close = false;
        let mut save = false;

        egui::Window::new("Rename device")
            .id(Id::new("rename-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    egui::RichText::new("Rename this device")
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "The new name is applied with hostnamectl and needs administrator approval.",
                    )
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
                ui.add_space(14.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut dialog.buf)
                        .font(regular(14.0))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(8, 6)),
                );
                if !dialog.busy && ctx.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                if let Some(err) = &dialog.error {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(err)
                            .font(regular(12.0))
                            .color(Color32::from_rgb(0xFF, 0x99, 0x99)),
                    );
                }
                if dialog.busy {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Waiting for authentication…")
                                .font(regular(12.0))
                                .color(pal.text_faint),
                        );
                    });
                }
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, pal, pal.accent, "Save", None).clicked() && !dialog.busy
                    {
                        save = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() && !dialog.busy {
                        close = true;
                    }
                });
            });

        if save && !dialog.busy {
            let name = dialog.buf.trim().to_owned();
            dialog.error = None;
            if name.is_empty() || name.len() > 63 {
                dialog.error = Some("Enter a name of 1–63 characters.".to_owned());
            } else if !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
            {
                dialog.error =
                    Some("Use only letters, digits, '-' and '.' (no spaces).".to_owned());
            } else {
                dialog.busy = true;
                self.rename_target = name.clone();
                let (tx, rx) = mpsc::channel();
                self.rename_rx = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(system::rename_hostname(&name));
                });
            }
        }

        if close && !dialog.busy {
            self.rename = None;
        }
    }

    fn dialogs(&mut self, ctx: &Context, pal: &Palette) {
        self.text_prompt_dialog(ctx, pal);
        self.rename_dialog(ctx, pal);
        self.wifi_dialog(ctx, pal);
        self.activity_log_dialog(ctx, pal);
        self.cleaner_dialog(ctx, pal);
        self.confirm_dialog(ctx, pal);
        crate::welcome::show(self, ctx, pal);
    }

    fn wifi_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(dialog) = self.wifi_connect.as_mut() else {
            return;
        };
        let mut connect = false;
        let mut close = false;
        egui::Window::new("Join Wi-Fi")
            .id(Id::new("wifi-connect-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    egui::RichText::new(format!("Join {}", dialog.ssid))
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(7.0);
                ui.label(
                    egui::RichText::new(if dialog.secured {
                        "Enter the Wi-Fi password. It is sent directly to NetworkManager and is not stored by Settings."
                    } else {
                        "This open network does not require a password."
                    })
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
                if dialog.secured {
                    ui.add_space(13.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut dialog.password)
                            .password(true)
                            .hint_text("Wi-Fi password")
                            .font(regular(14.0))
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(8, 6)),
                    );
                    if ctx.memory(|m| m.focused().is_none()) {
                        edit.request_focus();
                    }
                }
                if let Some(error) = &dialog.error {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(error)
                            .font(regular(12.0))
                            .color(Color32::from_rgb(0xFF, 0x99, 0x99)),
                    );
                }
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, pal, pal.accent, "Connect", None).clicked() {
                        connect = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.wifi_connect = None;
            return;
        }
        if connect {
            let (ssid, password) = {
                let dialog = self.wifi_connect.as_ref().expect("dialog exists");
                (
                    dialog.ssid.clone(),
                    if dialog.secured {
                        Some(dialog.password.clone())
                    } else {
                        None
                    },
                )
            };
            if password.as_deref().is_some_and(str::is_empty) {
                if let Some(dialog) = self.wifi_connect.as_mut() {
                    dialog.error = Some("Enter the Wi-Fi password.".to_owned());
                }
                return;
            }
            match system::connect_wifi(&ssid, password.as_deref()) {
                Ok(()) => {
                    self.wifi_connect = None;
                    self.notify(format!("Connected to {ssid}"));
                }
                Err(error) => {
                    if let Some(dialog) = self.wifi_connect.as_mut() {
                        dialog.error = Some(if error.is_empty() {
                            "NetworkManager could not join this network.".to_owned()
                        } else {
                            error
                        });
                    }
                }
            }
        }
    }


    fn cleaner_dialog(&mut self, ctx: &Context, pal: &Palette) {
        if !self.cleaner.open {
            return;
        }
        let scanning = self.cleaner.scanning;
        let error = self.cleaner.error.clone();
        let groups_empty = self.cleaner.groups.is_empty();
        let selected_bytes = self
            .cleaner
            .groups
            .iter()
            .map(|g| g.selected_size())
            .sum::<i64>();
        let mut close = false;
        let mut retry = false;
        let mut clean = false;
        let mut toggle_item: Option<(usize, usize)> = None;
        let mut toggle_group: Option<usize> = None;

        egui::Window::new("Free Up Space")
            .id(Id::new("free-up-space-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_width(600.0)
            .default_height(560.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(556.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Free Up Space")
                            .font(semibold(20.0))
                            .color(pal.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::ghost_button(ui, pal, "Close", None).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(
                        "What App Hub found: leftover files, unused packages and caches. Pick what to remove.",
                    )
                    .font(regular(12.0))
                    .color(pal.text_faint),
                );
                ui.add_space(10.0);

                if scanning {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Scanning for cleanable files… (one moment)")
                                .font(regular(13.0))
                                .color(pal.text_muted),
                        );
                    });
                } else if let Some(err) = &error {
                    widgets::card(ui, pal, |ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(err)
                                .font(regular(12.0))
                                .color(Color32::from_rgb(0xFF, 0x9B, 0x9B)),
                        );
                        ui.add_space(8.0);
                    });
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, pal, "Try again", Some(Icon::Update)).clicked() {
                        retry = true;
                    }
                } else if groups_empty {
                    widgets::card(ui, pal, |ui| {
                        widgets::empty_note(
                            ui,
                            pal,
                            "Nothing to clean — your system is tidy.",
                        );
                    });
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, pal, "Rescan", Some(Icon::Update)).clicked() {
                        retry = true;
                    }
                } else {
                    ScrollArea::vertical()
                        .id_salt("free-up-space-scroll")
                        .auto_shrink([false, false])
                        .max_height(400.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for (gi, group) in self.cleaner.groups.iter().enumerate() {
                                if gi > 0 {
                                    ui.add_space(10.0);
                                }
                                let size = cleaner::human_bytes(group.size());
                                ui.horizontal(|ui| {
                                    let master_checked = if group.granular {
                                        group.items.iter().all(|i| i.selected)
                                    } else {
                                        group.enabled
                                    };
                                    if cleaner_checkbox(ui, pal, master_checked)
                                        .clicked()
                                    {
                                        toggle_group = Some(gi);
                                    }
                                    ui.label(
                                        egui::RichText::new(&group.title)
                                            .font(semibold(13.5))
                                            .color(pal.text),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(&size)
                                                    .font(semibold(12.0))
                                                    .color(pal.text_faint),
                                            );
                                        },
                                    );
                                });
                                if !group.description.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&group.description)
                                            .font(regular(11.5))
                                            .color(pal.text_faint),
                                    );
                                }
                                if let Some(note) = (!group.note.is_empty()).then_some(&group.note) {
                                    ui.label(
                                        egui::RichText::new(note)
                                            .font(regular(11.0))
                                            .color(pal.text_faint),
                                    );
                                }
                                if group.granular {
                                    ui.add_space(4.0);
                                    widgets::card(ui, pal, |ui| {
                                        for (ii, item) in group.items.iter().enumerate() {
                                            if ii > 0 {
                                                widgets::divider(ui, pal);
                                            }
                                            if cleaner_item_row(ui, pal, item).clicked() {
                                                toggle_item = Some((gi, ii));
                                            }
                                        }
                                    });
                                } else {
                                    ui.add_space(2.0);
                                }
                            }
                        });

                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if selected_bytes > 0 {
                            format!("Clean Up · {}", cleaner::human_bytes(selected_bytes))
                        } else {
                            "Clean Up".to_owned()
                        };
                        let button = widgets::button(ui, pal, pal.accent, &label, Some(Icon::Trash));
                        if selected_bytes > 0 && button.clicked() {
                            clean = true;
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{} selected",
                                cleaner::human_bytes(selected_bytes)
                            ))
                            .font(regular(11.5))
                            .color(pal.text_faint),
                        );
                    });
                }
            });

        if close {
            self.cleaner.open = false;
        }
        if retry {
            self.cleaner.error = None;
            self.cleaner.groups.clear();
            self.cleaner_scan_start();
        }
        if let Some(gi) = toggle_group {
            if let Some(group) = self.cleaner.groups.get_mut(gi) {
                // Master checkbox toggles every item (granular) or the group itself.
                if group.granular {
                    let all = group.items.iter().all(|i| i.selected);
                    for item in group.items.iter_mut() {
                        item.selected = !all;
                    }
                } else {
                    group.enabled = !group.enabled;
                }
            }
        }
        if let Some((gi, ii)) = toggle_item {
            if let Some(item) = self.cleaner.groups.get_mut(gi).and_then(|g| g.items.get_mut(ii)) {
                item.selected = !item.selected;
            }
        }
        if clean {
            self.cleaner_clean();
        }
    }

    fn activity_log_dialog(&mut self, ctx: &Context, pal: &Palette) {
        if !self.activity_log_open {
            return;
        }
        // Capture the live job (if any) before borrowing `ui` mutably.
        let live = self
            .hub
            .job
            .as_ref()
            .map(|job| (job.label.clone(), job.log.clone()))
            .or_else(|| {
                self.apt_job
                    .as_ref()
                    .map(|job| (job.label.clone(), job.log.clone()))
            });
        egui::Window::new("Activity Log")
            .id(Id::new("activity-log-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .default_height(480.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Activity Log")
                            .font(semibold(20.0))
                            .color(pal.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::ghost_button(ui, pal, "Close", None).clicked() {
                            self.activity_log_open = false;
                        }
                        if !self.activity.is_empty()
                            && widgets::ghost_button(ui, pal, "Clear log", Some(Icon::Trash))
                                .clicked()
                        {
                            self.activity.clear();
                        }
                    });
                });
                ui.add_space(8.0);

                let bg = pal.input_bg;
                let border = pal.input_border;
                ScrollArea::vertical()
                    .id_salt("activity-log-scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .max_height(560.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::Frame::new()
                            .fill(bg)
                            .stroke(Stroke::new(1.0, border))
                            .corner_radius(4)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                if let Some((label, log)) = &live {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(
                                            egui::RichText::new(format!("{label}…"))
                                                .font(semibold(13.0))
                                                .color(pal.text),
                                        );
                                    });
                                    ui.add_space(6.0);
                                    if log.is_empty() {
                                        ui.label(
                                            egui::RichText::new(
                                                "Waiting for output…",
                                            )
                                            .font(regular(12.0))
                                            .color(pal.text_faint),
                                        );
                                    } else {
                                        for line in log.iter().take(400) {
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .font(regular(12.0))
                                                    .color(pal.text_faint),
                                            );
                                        }
                                    }
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(8.0);
                                }
                                let mut any = false;
                                for entry in self.activity.iter().rev().take(120) {
                                    any = true;
                                    let color = if entry.error {
                                        Color32::from_rgb(0xFF, 0x9B, 0x9B)
                                    } else {
                                        pal.text_faint
                                    };
                                    ui.label(
                                        egui::RichText::new(&entry.title)
                                            .font(semibold(12.5))
                                            .color(pal.text),
                                    );
                                    if !entry.details.trim().is_empty() {
                                        ui.label(
                                            egui::RichText::new(&entry.details)
                                                .font(regular(12.0))
                                                .color(color),
                                        );
                                    }
                                    ui.add_space(6.0);
                                }
                                if !any && live.is_none() {
                                    ui.label(
                                        egui::RichText::new(
                                            "No activity yet. Updates, installs and uninstalls will appear here with their full command output.",
                                        )
                                        .font(regular(12.0))
                                        .color(pal.text_faint),
                                    );
                                }
                            });
                    });
            });
    }

    fn confirm_dialog(&mut self, ctx: &Context, pal: &Palette) {
        let Some(dialog) = self.confirm.as_mut() else {
            return;
        };
        let mut accepted = false;
        let mut closed = false;

        egui::Window::new("Confirm")
            .id(Id::new("confirm-dialog"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.card_border))
                    .corner_radius(8)
                    .inner_margin(Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    egui::RichText::new(&dialog.title)
                        .font(semibold(20.0))
                        .color(pal.text),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&dialog.body)
                        .font(regular(12.5))
                        .color(pal.text_faint),
                );
                ui.add_space(18.0);
                // A fixed-height row: a bare right-to-left layout would take the
                // whole screen height and stretch the dialog.
                let row = Vec2::new(ui.available_width(), 34.0);
                ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::button(ui, pal, pal.accent, &dialog.confirm_label, None).clicked()
                        && !dialog.busy
                    {
                        accepted = true;
                    }
                    if widgets::ghost_button(ui, pal, "Cancel", None).clicked() && !dialog.busy {
                        closed = true;
                    }
                });
            });

        if closed && !dialog.busy {
            self.confirm = None;
            return;
        }
        if accepted && !dialog.busy {
            match &dialog.kind {
                ConfirmKind::InstallUpdates => {
                    self.confirm = None;
                    self.run_install_updates();
                }
                ConfirmKind::Uninstall { id, name } => {
                    let id = id.clone();
                    let _ = name;
                    self.confirm = None;
                    self.hub_act(HubAction::Remove, &id);
                }
                ConfirmKind::RemoveSysApp { app, .. } => {
                    let app = app.clone();
                    self.confirm = None;
                    let label = format!("Uninstalling {}", app.name);
                    self.record_activity(label.clone(), "Started.", false);
                    self.hub.job = Some(HubJob {
                        label,
                        log: Vec::new(),
                        rx: inventory::remove(&app),
                    });
                }
                ConfirmKind::ForgetWifi => {
                    self.confirm = None;
                    match system::forget_active_wifi() {
                        Ok(message) => {
                            self.notify(&message);
                            self.record_activity("Wi-Fi network forgotten", message, false);
                            self.reload_info();
                        }
                        Err(err) => self.notify_error(&err),
                    }
                }
                ConfirmKind::InstallCompat { package, name } => {
                    let package = package.clone();
                    let name = name.clone();
                    self.confirm = None;
                    self.run_compat_install(&package, &name);
                }
                ConfirmKind::InstallFavorite { key } => {
                    let key = key.clone();
                    self.confirm = None;
                    self.run_favorite_install(&key);
                }
                ConfirmKind::RemoveExtension { uuid } => {
                    let uuid = uuid.clone();
                    self.confirm = None;
                    crate::desktop::uninstall(self, ctx, uuid);
                }
            }
        }
    }

    fn toast(&mut self, ctx: &Context, pal: &Palette) {
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

    fn maybe_screenshot(&mut self, ctx: &Context) {
        let Some(path) = self.screenshot_path.clone() else {
            return;
        };
        let waiting_hub = matches!(self.view, View::Page(Page::Apps)) && self.hub.loading;
        let waiting_updates = self.updates_rx.is_some();
        // Testing hook: capture the very first frame, before the live probe
        // returns, to prove cached values are already visible.
        let early = std::env::var("WIN11_SETTINGS_SHOT_EARLY").is_ok();
        if !self.screenshot_requested
            && (early || (self.info_rx.is_none() && !waiting_hub && !waiting_updates))
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

    fn handle_resize(&self, ctx: &Context) {
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !ctx.input(|i| i.pointer.primary_pressed()) {
            return;
        }
        // A maximized or fullscreen window cannot be resized from its edges.
        let (maximized, fullscreen) = ctx.input(|i| {
            let vp = i.viewport();
            (vp.maximized.unwrap_or(false), vp.fullscreen.unwrap_or(false))
        });
        if maximized || fullscreen {
            return;
        }
        let direction = resize_direction(pos, ctx.viewport_rect());
        if let Some(direction) = direction {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
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

        if self.mode != self.last_mode || self.accent != self.last_accent {
            let pal = Palette::new(self.mode, self.accent);
            theme::apply_style(&ctx, &pal, self.mode);
            self.last_mode = self.mode;
            self.last_accent = self.accent;
        }

        let pal = self.pal();
        self.handle_resize(&ctx);

        let full = root.max_rect();
        root.painter().rect_filled(full, 0, pal.bg);

        let title_rect = Rect::from_min_size(full.min, Vec2::new(full.width(), TITLE_H));
        self.titlebar(root, &ctx, &pal, title_rect);

        let body = Rect::from_min_max(Pos2::new(full.left(), full.top() + TITLE_H), full.max);
        let sidebar_w = SIDEBAR_W.min(body.width() * 0.36).max(240.0);
        let side_rect = Rect::from_min_size(body.min, Vec2::new(sidebar_w, body.height()));
        let content_rect = Rect::from_min_max(Pos2::new(side_rect.right(), body.top()), body.max);

        self.sidebar(root, &pal, side_rect);
        self.content(root, &pal, content_rect);

        self.dialogs(&ctx, &pal);
        self.toast(&ctx, &pal);
        self.maybe_screenshot(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let pal = self.pal();
        [
            pal.bg.r() as f32 / 255.0,
            pal.bg.g() as f32 / 255.0,
            pal.bg.b() as f32 / 255.0,
            1.0,
        ]
    }
}

/// Small square checkbox used by the Free Up Space dialog.
fn cleaner_checkbox(ui: &mut Ui, pal: &Palette, checked: bool) -> egui::Response {
    let size = 18.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(size + 2.0, 26.0), Sense::click());
    let box_rect = Rect::from_center_size(Pos2::new(rect.left() + size / 2.0, rect.center().y), Vec2::splat(size));
    if checked {
        ui.painter().rect_filled(box_rect, 4, pal.accent);
        crate::icons::paint(ui.painter(), Icon::Check, box_rect.shrink(3.5), pal.on_accent);
    } else {
        ui.painter().rect_filled(box_rect, 4, pal.input_bg);
        ui.painter().rect_stroke(
            box_rect,
            4,
            Stroke::new(1.0, pal.input_border),
            StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// One selectable row inside a granular group (leftover files etc.).
fn cleaner_item_row(ui: &mut Ui, pal: &Palette, item: &cleaner::CleanerItem) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    if response.hovered() {
        let bg = Rect::from_min_max(
            Pos2::new(rect.left() + 2.0, rect.top() + 1.0),
            Pos2::new(rect.right() - 2.0, rect.bottom() - 1.0),
        );
        ui.painter().rect_filled(bg, 4, pal.hover);
    }
    let check = Rect::from_center_size(
        Pos2::new(rect.left() + 13.0, rect.center().y),
        Vec2::splat(17.0),
    );
    if item.selected {
        ui.painter().rect_filled(check, 4, pal.accent);
        crate::icons::paint(ui.painter(), Icon::Check, check.shrink(3.0), pal.on_accent);
    } else {
        ui.painter().rect_filled(check, 4, pal.input_bg);
        ui.painter().rect_stroke(
            check,
            4,
            Stroke::new(1.0, pal.input_border),
            StrokeKind::Inside,
        );
    }
    let size_text = cleaner::human_bytes(item.size);
    let galley = ui.painter().layout_no_wrap(
        size_text,
        regular(11.5),
        pal.text_faint,
    );
    let size_x = rect.right() - 10.0 - galley.size().x;
    ui.painter().galley(
        Pos2::new(size_x, rect.center().y - galley.size().y / 2.0),
        galley.clone(),
        pal.text_faint,
    );
    let label = if item.detail.is_empty() {
        item.label.clone()
    } else {
        format!("{}  ·  {}", item.label, item.detail)
    };
    ui.painter().text(
        Pos2::new(check.right() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        &label,
        regular(12.5),
        pal.text,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn pretty_id(id: &str) -> String {
    id.split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Split "host:port" (port optional); trims whitespace and brackets.
fn parse_host_port(raw: &str) -> (String, u32) {
    let raw = raw.trim();
    match raw.rsplit_once(':') {
        Some((host, port)) if port.parse::<u32>().is_ok() => (
            host.trim().trim_matches(['[', ']']).to_owned(),
            port.parse::<u32>().unwrap_or(0),
        ),
        _ => (raw.to_owned(), 0),
    }
}

/// Which window edge a press at `pos` should resize, if any. The caption
/// buttons reach the window edge, so their outer margin must click the
/// button, not start a resize (Close at the very corner, Windows 11 style).
fn resize_direction(pos: Pos2, screen: Rect) -> Option<egui::ResizeDirection> {
    let captions = Rect::from_min_max(
        Pos2::new(screen.right() - 46.0 * 3.0, screen.top()),
        Pos2::new(screen.right(), screen.top() + TITLE_H),
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

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))
    }

    #[test]
    fn close_button_margin_is_not_a_resize_edge() {
        for pos in [(1279.0, 0.0), (1279.0, 20.0), (1250.0, 0.0), (1145.0, 2.0), (1279.0, 47.0)] {
            assert_eq!(resize_direction(Pos2::new(pos.0, pos.1), screen()), None, "{pos:?}");
        }
    }

    #[test]
    fn edges_outside_captions_still_resize() {
        assert_eq!(resize_direction(Pos2::new(1279.0, 100.0), screen()), Some(D::East));
        assert_eq!(resize_direction(Pos2::new(500.0, 1.0), screen()), Some(D::North));
        assert_eq!(resize_direction(Pos2::new(1.0, 1.0), screen()), Some(D::NorthWest));
        assert_eq!(resize_direction(Pos2::new(1279.0, 799.0), screen()), Some(D::SouthEast));
        assert_eq!(resize_direction(Pos2::new(600.0, 400.0), screen()), None);
    }
}
