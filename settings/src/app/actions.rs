//! App methods that change a system setting (usually on a background thread).

use std::process::Command;
use std::sync::mpsc::{self};

use eframe::egui::Context;

use super::*;
use crate::system::{self};

impl App {
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
        self.run_sys(SysAction::WifiRadio(on), move || {
            system::set_wifi_enabled(on)
        });
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

    pub fn has_photo(&self) -> bool {
        self.photo.is_some()
    }

    /// Read the account picture in the background. With `change`, first
    /// set it: `Some("")` removes it, `Some("pick")` opens a file chooser.
    pub fn load_photo(&mut self, change: Option<&str>) {
        if self.photo_rx.is_some() {
            return;
        }
        let change = change.map(str::to_owned);
        if change.as_deref() == Some("pick") && !system::command_exists("zenity") {
            self.notify_error("zenity is not installed");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.photo_rx = Some(rx);
        std::thread::spawn(move || {
            if let Some(change) = change {
                let path = if change == "pick" {
                    let out = Command::new("zenity")
                        .args([
                            "--file-selection",
                            "--title=Choose your account picture",
                            "--file-filter=Images | *.png *.jpg *.jpeg",
                        ])
                        .output();
                    match out {
                        Ok(o) if o.status.success() => {
                            String::from_utf8_lossy(&o.stdout).trim().to_owned()
                        }
                        _ => {
                            let _ = tx.send(Err("cancelled".to_owned()));
                            return;
                        }
                    }
                } else {
                    change
                };
                if !path.is_empty() && system::load_photo(std::path::Path::new(&path), 8).is_none()
                {
                    let _ = tx.send(Err("That file is not a PNG or JPEG picture".to_owned()));
                    return;
                }
                if let Err(err) = system::set_user_photo(&path) {
                    let _ = tx.send(Err(err));
                    return;
                }
            }
            let photo = system::user_photo().and_then(|p| system::load_photo(&p, 256));
            let _ = tx.send(Ok(photo));
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

    pub(crate) fn apply_wallpaper(&mut self, path: &str) {
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

    /// The glass style this window paints with.
    pub fn glass(&self) -> crate::theme::Glass {
        if let Ok(v) = std::env::var("WIN11_SETTINGS_GLASS") {
            return crate::theme::Glass::from_text(&v);
        }
        self.resolved_glass(system::GlassScope::Settings)
    }

    pub(crate) fn scope_index(scope: system::GlassScope) -> usize {
        system::GlassScope::ALL
            .iter()
            .position(|s| *s == scope)
            .unwrap_or(0)
    }

    /// A scope's own values, or `None` when it follows the scope above.
    pub fn scope_glass(&self, scope: system::GlassScope) -> Option<crate::theme::Glass> {
        self.glass_scopes[Self::scope_index(scope)]
    }

    /// The values a scope paints with, after following its parents.
    pub fn resolved_glass(&self, scope: system::GlassScope) -> crate::theme::Glass {
        let mut at = scope;
        loop {
            if let Some(g) = self.scope_glass(at) {
                return g;
            }
            match at.parent() {
                Some(p) => at = p,
                None => return crate::theme::Glass::OFF,
            }
        }
    }

    /// Change a scope's glass (`None` = follow the scope above). The global
    /// scope is written to Fusion Desk at once.
    pub fn set_scope_glass(
        &mut self,
        scope: system::GlassScope,
        glass: Option<crate::theme::Glass>,
        persist: bool,
    ) {
        self.glass_scopes[Self::scope_index(scope)] = glass;
        if !persist {
            self.glass_dirty = Some(scope);
            return;
        }
        self.glass_dirty = None;
        if !system::save_scope_glass(scope, glass) {
            self.notify_error("Fusion Desk did not take the glass settings; update Fusion Desk");
        }
    }

    /// Read the global glass again (Fusion Desk may have changed it).
    pub fn reload_global_glass(&mut self) {
        self.glass_scopes[0] = Some(system::global_glass());
    }

    pub fn set_home_links(&mut self, on: bool) {
        self.home_links = on;
        system::save_home_links(on);
    }

    /// Switch the interface design. Only the presentation changes: every
    /// setting, page and background job is shared by both designs.
    pub fn set_design(&mut self, design: crate::theme::Design) {
        if self.design == design {
            return;
        }
        self.design = design;
        system::save_design(design);
        self.search.clear();
        self.view = match design {
            crate::theme::Design::Win10 => View::Home,
            crate::theme::Design::Win11 => View::Page(Page::System),
        };
        self.notify(format!("{} interface applied", design.label()));
    }
}

/// Split "host:port" (port optional); trims whitespace and brackets.
pub(crate) fn parse_host_port(raw: &str) -> (String, u32) {
    let raw = raw.trim();
    match raw.rsplit_once(':') {
        Some((host, port)) if port.parse::<u32>().is_ok() => (
            host.trim().trim_matches(['[', ']']).to_owned(),
            port.parse::<u32>().unwrap_or(0),
        ),
        _ => (raw.to_owned(), 0),
    }
}
