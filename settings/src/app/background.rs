//! Background work: loading `Info`, polling job channels, toasts and the activity log.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Context, Id};

use super::*;
use crate::hub::{self};
use crate::system::{self, Info};

impl App {
    /// Screenshot runs only: show a job panel with a sample log.
    /// `apt`, `apt-done`, `hub`, `hub-error`.
    pub(crate) fn fake_job(&mut self, kind: &str) {
        let log: Vec<String> = [
            "Reading package lists...",
            "Building dependency tree...",
            "The following packages will be upgraded:",
            "  firefox libgtk-4-1 mesa-vulkan-drivers",
            "3 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.",
            "Need to get 84.6 MB of archives.",
            "dlstatus:1:0.0000:Retrieving file 1 of 3",
            "Get:1 http://archive.ubuntu.com/ubuntu resolute-updates/main amd64 firefox 1.2 [71.0 MB]",
            "dlstatus:2:62.4000:Retrieving file 2 of 3 (8,412 kB/s 3s remaining)",
        ]
        .iter()
        .map(|l| (*l).to_owned())
        .collect();
        let (tx, rx) = mpsc::channel();
        std::mem::forget(tx);
        match kind {
            "apt" => {
                self.apt_job = Some(AptJob {
                    label: "Installing updates".to_owned(),
                    log,
                    rx,
                })
            }
            "apt-done" => {
                self.apt_done = Some(crate::jobview::DoneJob::new(
                    "Installing updates",
                    &log.join("\n"),
                    true,
                ))
            }
            "hub" => {
                self.hub.job = Some(HubJob {
                    label: "Uninstalling GIMP".to_owned(),
                    log: vec![
                        "$ apt-get remove -y gimp".to_owned(),
                        "Reading package lists...".to_owned(),
                        "Removing gimp (3.0.4-1) ...".to_owned(),
                        "pmstatus:gimp:48.0:Removing gimp (amd64)".to_owned(),
                    ],
                    rx,
                })
            }
            _ => {
                self.hub_done = Some(crate::jobview::DoneJob::new(
                    "Uninstalling GIMP",
                    "$ apt-get remove -y gimp\nE: Could not get lock /var/lib/dpkg/lock-frontend",
                    false,
                ))
            }
        }
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
    pub(crate) fn drain_job(
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
    pub(crate) fn finish_job(
        &mut self,
        label: &str,
        code: i32,
        log: String,
        success: bool,
        hub_job: bool,
    ) {
        let log = log
            .lines()
            .filter(|l| !crate::jobview::is_status_line(l))
            .collect::<Vec<_>>()
            .join("\n");
        let done = crate::jobview::DoneJob::new(label, &log, success);
        if hub_job {
            self.hub_done = Some(done);
        } else {
            self.apt_done = Some(done);
        }
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
            // The installed-apps list was loaded once; read it again so
            // an uninstalled (or newly installed) app shows the change.
            if let Some((channel, id, desktop)) = self.hub.sys_removing.take() {
                if success {
                    self.hub
                        .sys_apps
                        .retain(|a| !(a.channel == channel && a.id == id && a.desktop == desktop));
                }
            }
            self.reload_sys_apps();
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

    /// Apply the visible effect of a completed settings write.
    pub(crate) fn finish_sys(&mut self, action: SysAction) {
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
                self.notify(if on {
                    "Wi-Fi turned on"
                } else {
                    "Wi-Fi turned off"
                });
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

    pub(crate) fn poll_background(&mut self, ctx: &Context) {
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
                            let inline =
                                matches!(action, SysAction::RealName(_) | SysAction::Password);
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
                    self.hub.sys_error =
                        Some("The installed-apps scan stopped unexpectedly.".to_owned());
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

        if self.photo_rx.is_some() {
            let received = self.photo_rx.as_ref().map(|rx| rx.try_recv());
            match received {
                Some(Ok(result)) => {
                    self.photo_rx = None;
                    match result {
                        Ok(Some((side, pixels))) => {
                            let image =
                                egui::ColorImage::from_rgba_unmultiplied([side, side], &pixels);
                            self.photo = Some(ctx.load_texture(
                                "user-photo",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                        Ok(None) => self.photo = None,
                        Err(err) if err == "cancelled" => {}
                        Err(err) => self.notify_error(&err),
                    }
                }
                Some(Err(mpsc::TryRecvError::Empty)) => {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => self.photo_rx = None,
                None => {}
            }
        }
        // Every avatar reads the picture from here.
        match &self.photo {
            Some(tex) => {
                let id = tex.id();
                ctx.data_mut(|d| d.insert_temp(Id::new("user-photo"), id));
            }
            None => ctx.data_mut(|d| d.remove::<egui::TextureId>(Id::new("user-photo"))),
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
}
