//! App methods for the Apps page: App Hub installs, updates, removal, cleaner.

use std::sync::mpsc::{self};

use super::*;
use crate::cleaner;
use crate::hub::{self};
use crate::inventory;
use crate::system::{self};

impl App {
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

    /// Read the installed-apps list again in the background. The old list
    /// stays on screen until the new one arrives.
    pub fn reload_sys_apps(&mut self) {
        if self.hub.sys_apps.is_empty() && self.hub.sys_rx.is_none() {
            // Never loaded: the All Apps tab loads it when opened.
            return;
        }
        self.hub.sys_error = None;
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
