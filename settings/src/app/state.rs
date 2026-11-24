//! Plain state types for dialogs, background jobs, toasts and the App Hub.

use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::cleaner;
use crate::hub::{self, HubApp};
use crate::inventory;

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
    pub(crate) rx: Receiver<crate::jobs::JobEvent>,
}

#[derive(Default)]
pub struct CleanerState {
    pub open: bool,
    pub scanning: bool,
    pub groups: Vec<cleaner::CleanerGroup>,
    pub error: Option<String>,
    pub(crate) rx: Option<Receiver<Result<Vec<cleaner::CleanerGroup>, String>>>,
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
    pub(crate) rx: Receiver<crate::jobs::JobEvent>,
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
    /// The app being uninstalled: (channel, id, desktop file). Dropped from
    /// `sys_apps` as soon as the uninstall succeeds.
    pub sys_removing: Option<(String, String, String)>,
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
    pub(crate) rx: Option<
        Receiver<(
            String,
            Result<Vec<HubApp>, String>,
            Result<Vec<hub::LibraryApp>, String>,
            Vec<hub::Favorite>,
        )>,
    >,
    pub(crate) sys_rx: Option<Receiver<Result<Vec<inventory::SysApp>, String>>>,
    pub job: Option<HubJob>,
}

impl Default for AppsTab {
    fn default() -> Self {
        AppsTab::AllApps
    }
}

pub(crate) struct Toast {
    pub(crate) text: String,
    pub(crate) until: Instant,
    pub(crate) error: bool,
}
