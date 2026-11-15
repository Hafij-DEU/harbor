//! Integration with App Hub (github.com/Hafij-DEU/Application-hub), the system
//! app manager: the GitHub-release catalogue (`app-hub list`) and the Library
//! of extra GitHub projects it can install. Everything else in the app —
//! the installed-apps inventory, uninstalls and the cleaner — is implemented
//! natively (see `crate::inventory` and `crate::cleaner`) and works without
//! App Hub. The catalogue tabs hide themselves when App Hub is absent.

use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;

use crate::jobs::{self, JobEvent};
use crate::system::command_exists;

#[derive(Clone)]
pub struct HubApp {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub installed: Option<String>,
    pub latest: String,
    pub status: String,
    /// Absolute icon path reported by App Hub, when it ships one.
    pub icon: Option<String>,
    /// Why the last check failed, if it did (`app-hub list --json`).
    pub error: Option<String>,
    /// The cached release information is out of date.
    pub stale: bool,
}

impl HubApp {
    pub fn update_available(&self) -> bool {
        self.status.eq_ignore_ascii_case("update available")
    }

    pub fn installable(&self) -> bool {
        !self.update_available()
            && self.installed.is_none()
            && self.error.is_none()
            && !self.status.contains("no .deb")
            && !self.status.contains("error")
    }

    pub fn ready(&self) -> bool {
        self.installed.is_some() && !self.update_available()
    }

    pub fn installed_label(&self) -> String {
        self.installed.clone().unwrap_or_else(|| "—".to_owned())
    }
}

pub struct CatalogEntry {
    pub name: String,
    pub summary: String,
}

pub fn available() -> bool {
    command_exists("app-hub")
}

pub fn version() -> String {
    let out = Command::new("app-hub").arg("--version").output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout)
            .trim()
            .trim_start_matches("app-hub ")
            .to_owned(),
        Err(_) => String::new(),
    }
}

/// Read the catalogue shipped with the installed package (name + summary).
pub fn catalog() -> HashMap<String, CatalogEntry> {
    let mut map = HashMap::new();
    let paths = [
        "/usr/lib/app-hub/catalog.json",
        "/usr/share/app-hub/catalog.json",
    ];
    for path in paths {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(apps) = json.get("apps").and_then(|a| a.as_array()) else {
            continue;
        };
        for app in apps {
            let Some(id) = app.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            map.insert(
                id.to_owned(),
                CatalogEntry {
                    name: app
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(id)
                        .to_owned(),
                    summary: app
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                },
            );
        }
        break;
    }
    map
}

/// Ids that belong in "My Apps": the shipped base catalogue (the user's own
/// projects) plus anything the user added to My Apps (`section == "mine"`).
/// The default Library entries — third-party "craft" apps from other repos —
/// are deliberately excluded and stay in the Library tab.
pub fn my_app_ids(library: &[LibraryApp]) -> HashSet<String> {
    let mut ids: HashSet<String> = catalog().keys().cloned().collect();
    for app in library {
        if app.section == "mine" {
            ids.insert(app.id.clone());
        }
    }
    ids
}

/// `app-hub list --json` (App Hub 1.7 and newer), falling back to the plain
/// table on older releases. Network access happens inside App Hub.
pub fn list() -> Result<Vec<HubApp>, String> {
    let output = Command::new("app-hub")
        .args(["list", "--json"])
        .stdin(Stdio::null())
        .output();
    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(apps) = parse_json_list(&text) {
                return Ok(apps);
            }
        }
    }

    let output = Command::new("app-hub")
        .arg("list")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not run app-hub: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if err.is_empty() {
            "App Hub could not list applications.".to_owned()
        } else {
            err
        });
    }
    let catalog = catalog();
    let text = String::from_utf8_lossy(&output.stdout);
    let mut apps = Vec::new();
    for line in text.lines().skip(1) {
        let cols = line.split_whitespace().collect::<Vec<_>>();
        if cols.len() < 4 {
            continue;
        }
        let id = cols[0].to_owned();
        let installed = (cols[1] != "-").then(|| cols[1].to_owned());
        let latest = cols[2].to_owned();
        let status = cols[3..].join(" ");
        let entry = catalog.get(&id);
        apps.push(HubApp {
            name: entry
                .map(|e| e.name.clone())
                .unwrap_or_else(|| pretty_id(&id)),
            summary: entry.map(|e| e.summary.clone()).unwrap_or_default(),
            id,
            installed,
            latest,
            status,
            icon: None,
            error: None,
            stale: false,
        });
    }
    sort_apps(&mut apps);
    Ok(apps)
}

fn sort_apps(apps: &mut [HubApp]) {
    apps.sort_by(|a, b| {
        b.update_available()
            .cmp(&a.update_available())
            .then_with(|| b.installable().cmp(&a.installable()))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// Parse the machine-readable list App Hub 1.7.15 provides. Names, summaries,
/// icons and per-app errors come straight from App Hub instead of the table.
fn parse_json_list(text: &str) -> Option<Vec<HubApp>> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let items = json.get("apps")?.as_array()?;
    let catalog = catalog();
    let mut apps = Vec::new();
    for item in items {
        let id = item.get("id").and_then(|v| v.as_str())?.to_owned();
        let field = |key: &str| {
            item.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_owned()
        };
        let entry = catalog.get(&id);
        let installed = item
            .get("installed")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .filter(|v| !v.is_empty());
        let status = {
            let reported = field("status");
            if reported.is_empty() {
                if installed.is_none() {
                    "not installed".to_owned()
                } else {
                    "up to date".to_owned()
                }
            } else {
                reported
            }
        };
        let name = {
            let reported = field("name");
            if reported.is_empty() {
                entry
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| pretty_id(&id))
            } else {
                reported
            }
        };
        let summary = {
            let reported = field("summary");
            if reported.is_empty() {
                entry.map(|e| e.summary.clone()).unwrap_or_default()
            } else {
                reported
            }
        };
        let icon = field("icon");
        let error = field("error");
        apps.push(HubApp {
            id,
            name,
            summary,
            installed,
            latest: field("latest"),
            status,
            icon: (!icon.is_empty()).then_some(icon),
            error: (!error.is_empty()).then_some(error),
            stale: item.get("stale").and_then(|v| v.as_bool()).unwrap_or(false),
        });
    }
    sort_apps(&mut apps);
    Some(apps)
}

fn pretty_id(id: &str) -> String {
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

/// Install or update a single catalogue app. App Hub handles downloads and asks
/// for authorisation through polkit, exactly like its own window does.
pub fn run_action(action: Action, id: &str) -> Receiver<JobEvent> {
    let mut cmd = Command::new("app-hub");
    cmd.arg(match action {
        Action::Install => "install",
        Action::Update => "update",
        Action::Remove => "remove",
        Action::Open => "open",
    })
    .arg(id)
    .stdin(Stdio::null());
    jobs::spawn(cmd)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Remove,
    Open,
}

impl Action {
    pub fn verb(self) -> &'static str {
        match self {
            Action::Install => "Installing",
            Action::Update => "Updating",
            Action::Remove => "Uninstalling",
            Action::Open => "Opening",
        }
    }
}

/// Update every App Hub application in one go (`app-hub update-all --apps-only`).
/// System packages are handled by the Software updates page.
pub fn update_all() -> Receiver<JobEvent> {
    let mut cmd = Command::new("app-hub");
    cmd.args(["update-all", "--apps-only", "--yes"])
        .stdin(Stdio::null());
    jobs::spawn(cmd)
}


// ---------------------------------------------------------------- favorites
// App Hub 1.7 saves install recipes on a "Favorites" page: apt, snap, flatpak,
// catalogue, GitHub .deb, direct .deb and AppImage. They are not part of the
// catalogue or the Library, so without this section they were invisible here.

/// One saved install recipe from App Hub's Favorites.
#[derive(Clone, Default)]
pub struct Favorite {
    pub channel: String,
    pub id: String,
    pub name: String,
    pub summary: String,
    pub remote: String,
    pub branch: String,
    pub classic: bool,
    pub url: String,
    pub package: String,
    pub icon: String,
}

impl Favorite {
    pub fn key(&self) -> String {
        format!("{}:{}", self.channel, self.id)
    }

    pub fn channel_label(&self) -> &'static str {
        match self.channel.as_str() {
            "apt" => "Deb",
            "snap" => "Snap",
            "flatpak" => "Flatpak",
            "hub" => "App Hub",
            "github" => "GitHub .deb",
            "deb-url" => ".deb download",
            "appimage-url" => "AppImage",
            _ => "App",
        }
    }

    /// The install this recipe performs, worded the way App Hub words it.
    pub fn describe(&self) -> String {
        match self.channel.as_str() {
            "apt" => format!("apt install {}", self.id),
            "snap" => {
                let mut text = format!("snap install {}", self.id);
                if !self.remote.is_empty() {
                    text.push_str(&format!(" --channel={}", self.remote));
                }
                if self.classic {
                    text.push_str(" --classic");
                }
                text
            }
            "flatpak" => {
                // App Hub installs `id//branch` when a branch is recorded.
                let id = if self.branch.is_empty() || self.branch == "stable" {
                    self.id.clone()
                } else {
                    format!("{}//{}", self.id, self.branch)
                };
                format!(
                    "flatpak install {} {}",
                    if self.remote.is_empty() {
                        "flathub"
                    } else {
                        &self.remote
                    },
                    id
                )
            }
            "hub" => format!("App Hub: latest release of {}", self.name),
            "github" => format!("latest .deb from github.com/{}/releases", self.id),
            "deb-url" => self.url.clone(),
            "appimage-url" => format!("AppImage from {}", self.url),
            other => format!("{other} {}", self.id),
        }
    }

    /// Channels this app can install itself. Downloads that need App Hub's
    /// helper (GitHub .deb, direct .deb, AppImage) stay with App Hub.
    pub fn installable(&self) -> bool {
        matches!(self.channel.as_str(), "apt" | "snap" | "flatpak" | "hub")
    }
}

/// Read `favorites.json` from App Hub's data directory.
pub fn load_favorites() -> Vec<Favorite> {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| format!("{home}/.local/share"))
        })
        .unwrap_or_default();
    if base.is_empty() {
        return Vec::new();
    }
    let path = std::path::Path::new(&base).join("app-hub/favorites.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(items) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(items) = items.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let text = |key: &str| {
                item.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned()
            };
            let channel = text("channel");
            let id = text("id");
            if channel.is_empty() || id.is_empty() {
                return None;
            }
            Some(Favorite {
                channel,
                id,
                name: text("name"),
                summary: text("summary"),
                remote: text("remote"),
                branch: text("branch"),
                classic: item
                    .get("classic")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                url: text("url"),
                package: text("package"),
                icon: text("icon"),
            })
        })
        .collect()
}

/// Install one saved favorite. apt gets the same PolicyKit path as the
/// compatibility packages; snap/flatpak run their own client as root.
pub fn run_favorite(favorite: &Favorite) -> Receiver<JobEvent> {
    match favorite.channel.as_str() {
        "hub" => {
            let mut cmd = Command::new("app-hub");
            cmd.arg("install").arg(&favorite.id).stdin(Stdio::null());
            jobs::spawn(cmd)
        }
        "apt" => {
            let package = if favorite.package.is_empty() {
                favorite.id.clone()
            } else {
                favorite.package.clone()
            };
            crate::system::install_package(&package)
        }
        "snap" => {
            let mut cmd = Command::new("pkexec");
            cmd.args(["snap", "install", &favorite.id]);
            if !favorite.remote.is_empty() {
                cmd.arg(format!("--channel={}", favorite.remote));
            }
            if favorite.classic {
                cmd.arg("--classic");
            }
            cmd.stdin(Stdio::null());
            jobs::spawn(cmd)
        }
        "flatpak" => {
            let mut cmd = Command::new("pkexec");
            cmd.args(["flatpak", "install", "-y", "--noninteractive"]);
            cmd.arg(if favorite.remote.is_empty() {
                "flathub"
            } else {
                favorite.remote.as_str()
            });
            if favorite.branch.is_empty() || favorite.branch == "stable" {
                cmd.arg(&favorite.id);
            } else {
                cmd.arg(format!("{}//{}", favorite.id, favorite.branch));
            }
            cmd.stdin(Stdio::null());
            jobs::spawn(cmd)
        }
        _ => {
            // These downloads go through App Hub's own helper; explain instead
            // of half-installing something.
            let message = format!("Install this one from App Hub: {}", favorite.describe());
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(JobEvent::Line(message.clone()));
                let _ = tx.send(JobEvent::Finished { code: 1, log: message });
            });
            rx
        }
    }
}

// ----------------------------------------------------------------- library
// The Library: GitHub projects that publish .deb releases, grouped by owner.
// Read through App Hub's own module so it honours the same shipped defaults,
// user additions (`app-hub add`) and removals; hidden entirely without App Hub.

/// One Library entry (an extra GitHub project with .deb releases).
#[derive(Clone, Default)]
pub struct LibraryApp {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub repo: String,
    pub owner: String,
    /// Absolute icon path (shipped or downloaded by App Hub), if any.
    pub icon: Option<String>,
    /// "mine" = promoted to My Apps; anything else stays in the Library.
    pub section: String,
}

/// Load the Library. Returns an empty list when App Hub is absent.
pub fn load_library() -> Result<Vec<LibraryApp>, String> {
    if !available() {
        return Ok(Vec::new());
    }
    let script = r#"
import json, sys
sys.path.insert(0, "/usr/lib/app-hub")
try:
    from app_hub import library
    apps = library.load_apps()
except Exception as e:
    sys.stderr.write(str(e))
    sys.exit(1)
print(json.dumps([{
    "id": a.id,
    "name": a.name,
    "summary": a.summary,
    "repo": a.repo,
    "owner": getattr(a, "owner", "") or a.repo.split("/")[0],
    "icon": getattr(a, "icon", "") or "",
    "section": getattr(a, "section", "") or "",
} for a in apps]))
"#;
    let output = Command::new("python3")
        .args(["-Is", "-c", script])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not read the Library: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if err.is_empty() {
            "Could not read the Library.".to_owned()
        } else {
            err
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(text.trim())
        .map_err(|e| format!("Could not parse the Library: {e}"))?;
    let Some(items) = json.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .map(|item| {
            let text = |key: &str| {
                item.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned()
            };
            let icon = text("icon");
            LibraryApp {
                id: text("id"),
                name: text("name"),
                summary: text("summary"),
                repo: text("repo"),
                owner: text("owner"),
                icon: if icon.is_empty() { None } else { Some(icon) },
                section: text("section"),
            }
        })
        .collect())
}

/// Add a GitHub project to the Library (`app-hub add <owner/repo>`;
/// URLs work too — App Hub normalises them). Streams output into the activity log.
pub fn add_library_app(spec: &str) -> Receiver<JobEvent> {
    let mut cmd = Command::new("app-hub");
    cmd.args(["add", spec]);
    cmd.stdin(Stdio::null());
    jobs::spawn(cmd)
}
