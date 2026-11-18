//! Everything that talks to the system: probing for information and applying
//! settings through the standard Ubuntu tools (gsettings, pactl, nmcli, ...).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::theme::Mode;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PackageUpdate {
    pub name: String,
    pub old: String,
    pub new: String,
    /// "apt", "snap" or "flatpak" (empty means apt, for old snapshots).
    pub source: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sink {
    pub name: String,
    pub description: String,
    /// WirePlumber node id, used when only wpctl is available.
    pub id: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BluetoothDevice {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WifiNetwork {
    pub ssid: String,
    pub security: String,
    pub signal: u8,
    pub active: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetDevice {
    pub name: String,
    pub kind: String,
    pub state: String,
    pub connection: String,
    pub address: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mount {
    pub path: String,
    pub used_gb: f32,
    pub total_gb: f32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayMode {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub rate: f32,
    pub scales: Vec<f32>,
    pub current: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayMonitor {
    pub connector: String,
    pub scale: f32,
    pub modes: Vec<DisplayMode>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Info {
    pub loading: bool,

    pub hostname: String,
    pub user: String,
    pub full_name: String,
    pub is_admin: bool,
    pub groups: Vec<String>,
    pub home: String,
    pub shell: String,

    pub os_pretty: String,
    pub os_short: String,
    pub kernel: String,
    pub desktop: String,
    pub vendor: String,
    pub product: String,

    pub cpu: String,
    pub mem_total_gb: f32,
    pub mem_available_gb: f32,
    pub disk_free_gb: f32,
    pub disk_total_gb: f32,
    pub disk_used_pct: f32,
    pub home_used_gb: Option<f32>,
    pub mounts: Vec<Mount>,
    pub uptime: String,

    pub resolution: String,
    pub refresh: String,
    pub scale: String,
    pub brightness: Option<u8>,

    pub volume: u32,
    pub muted: bool,
    pub default_sink: String,
    pub sinks: Vec<Sink>,

    pub notifications: bool,
    pub lockscreen_notifications: bool,
    pub night_light: bool,
    pub idle_delay: u32,

    // Appearance / clock details picked up from GNOME.
    pub clock_show_date: bool,
    pub clock_show_weekday: bool,
    pub clock_show_seconds: bool,
    pub battery_percentage: bool,

    // Mouse & touchpad.
    pub mouse_speed: f32,
    pub mouse_natural: bool,
    pub mouse_left_handed: bool,
    pub touchpad_tap: bool,
    pub touchpad_natural: bool,
    pub touchpad_disable_typing: bool,
    pub touchpad_two_finger: bool,
    pub touchpad_speed: f32,

    // Keyboard.
    pub key_repeat: bool,
    pub repeat_delay: u32,
    pub repeat_interval: u32,
    pub input_sources: Vec<String>,

    // Power: suspend on idle and lid behaviour.
    pub sleep_ac: bool,
    pub sleep_battery: bool,
    pub lid_ac: String,
    pub lid_battery: String,

    // Sound input.
    pub microphone_volume: u32,
    pub input_muted: bool,
    pub sources: Vec<Sink>,
    pub default_source: String,

    /// Automatic (unattended) security updates; `None` when not available.
    pub automatic_updates: Option<bool>,

    /// Desktop ids of the apps that handle web/mailto/text, plus candidate lists.
    pub default_browser: String,
    pub default_mail: String,
    pub default_editor: String,
    pub browsers: Vec<(String, String)>,
    pub mail_clients: Vec<(String, String)>,
    pub editors: Vec<(String, String)>,
    pub mime_defaults: Vec<MimeDefault>,

    // Display modes reported by the compositor.
    pub display_monitors: Vec<DisplayMonitor>,

    // Night light schedule.
    pub night_light_auto: bool,
    /// Whether night light follows a time window at all. When false the
    /// setting is "on all day" and the switch warms the screen immediately.
    pub night_light_scheduled: bool,
    pub night_light_from: f32,
    pub night_light_to: f32,
    pub night_light_temp: u32,

    // Network extras.
    pub wifi_enabled: bool,
    pub proxy_mode: String,
    pub proxy_http_host: String,
    pub proxy_http_port: u32,
    pub proxy_socks_host: String,
    pub proxy_socks_port: u32,
    pub proxy_ignore: String,

    // Power extras.
    pub power_button: String,
    pub low_battery_saver: bool,

    // Bluetooth extras.
    pub discoverable: bool,

    // Time.
    pub ntp_enabled: bool,

    // Personalisation.
    pub cursor_size: u32,

    // Accounts; None when the login manager config cannot be read.
    pub autologin: Option<bool>,

    pub battery: Option<u8>,
    pub charging: bool,
    pub power_profile: String,

    pub clock_24h: bool,
    pub timezone: String,
    pub locale: String,
    pub local_time: String,
    pub wallpaper: String,

    pub bluetooth_on: bool,
    pub bluetooth_devices: Vec<BluetoothDevice>,
    pub net_devices: Vec<NetDevice>,
    pub wifi_networks: Vec<WifiNetwork>,

    pub wine: Option<String>,
    pub waydroid: bool,
    pub steam: bool,
    pub updates: Option<Vec<PackageUpdate>>,
}

fn cache_path() -> Option<std::path::PathBuf> {
    let base = std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.cache")))?;
    Some(std::path::Path::new(&base).join("win11-settings/info.json"))
}

/// The snapshot written by the previous launch. It lets the first frame show
/// real storage, hardware and network values instead of "…" while the live
/// probe runs in the background.
pub fn cached_info() -> Option<Info> {
    if std::env::var("WIN11_SETTINGS_NOCACHE").is_ok() {
        return None;
    }
    let path = cache_path()?;
    let text = fs::read_to_string(path).ok()?;
    let mut info: Info = serde_json::from_str(&text).ok()?;
    info.loading = false;
    info.updates = None;
    Some(info)
}

fn save_cache(info: &Info) {
    if std::env::var("WIN11_SETTINGS_NOCACHE").is_ok() {
        return;
    }
    let Some(path) = cache_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let Ok(text) = serde_json::to_string(info) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, text).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

impl Info {
    pub fn placeholder() -> Self {
        Self {
            loading: true,
            hostname: "…".to_owned(),
            user: "…".to_owned(),
            full_name: "…".to_owned(),
            os_pretty: "…".to_owned(),
            os_short: "…".to_owned(),
            kernel: "…".to_owned(),
            desktop: "…".to_owned(),
            cpu: "…".to_owned(),
            uptime: "…".to_owned(),
            resolution: "…".to_owned(),
            refresh: "…".to_owned(),
            scale: "100%".to_owned(),
            default_sink: "…".to_owned(),
            power_profile: "…".to_owned(),
            timezone: "…".to_owned(),
            locale: "…".to_owned(),
            local_time: "…".to_owned(),
            wallpaper: String::new(),
            ..Default::default()
        }
    }

    pub fn load() -> Self {
        // One bulk read of every GSettings value instead of one process per key.
        dconf_begin();

        let mut info = Info::default();
        info.loading = false;

        info.user = std::env::var("USER").unwrap_or_else(|_| sh("id -un"));
        info.full_name = getent_full_name(&info.user);
        if info.full_name.is_empty() {
            info.full_name = info.user.clone();
        }
        info.hostname = sh("hostname");
        info.home = std::env::var("HOME").unwrap_or_default();
        info.shell = std::env::var("SHELL").unwrap_or_default();
        info.groups = sh("id -nG").split_whitespace().map(str::to_owned).collect();
        info.is_admin = info
            .groups
            .iter()
            .any(|g| matches!(g.as_str(), "sudo" | "admin" | "wheel"));

        let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
        info.os_pretty = os_field(&os, "PRETTY_NAME");
        let id = os_field(&os, "ID");
        let version = os_field(&os, "VERSION_ID");
        info.os_short = if id.is_empty() {
            info.os_pretty.clone()
        } else {
            format!("{} {}", capitalize(&id), version)
        };
        if info.os_short.trim().is_empty() {
            info.os_short = "Ubuntu".to_owned();
        }

        info.kernel = sh("uname -r");
        info.desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .map(|s| human_desktop(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Ubuntu".to_owned());
        info.vendor = read_trim("/sys/class/dmi/id/sys_vendor");
        info.product = read_trim("/sys/class/dmi/id/product_name");

        info.cpu = cpu_model();
        if let Some((total, avail)) = memory() {
            info.mem_total_gb = total;
            info.mem_available_gb = avail;
        }
        let (total, free, pct) = root_disk();
        info.disk_total_gb = total;
        info.disk_free_gb = free;
        info.disk_used_pct = pct;
        // Home usage walks the whole tree (seconds on a large disk): the app
        // runs it as a separate background probe instead of blocking startup.
        info.mounts = mounts();
        info.uptime = sh("uptime -p").trim_start_matches("up ").to_owned();

        info.display_monitors = display_monitors();
        let (resolution, refresh) = display_mode(&info.display_monitors);
        info.resolution = resolution;
        info.refresh = refresh;
        info.scale = text_scale();
        info.brightness = brightness();

        let (volume, muted) = audio_state();
        info.volume = volume;
        info.muted = muted;
        info.sinks = sinks();
        info.default_sink = default_sink_name();
        if info.default_sink.is_empty() {
            info.default_sink = info
                .sinks
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_default();
        }

        info.notifications =
            gs_bool("org.gnome.desktop.notifications", "show-banners").unwrap_or(true);
        info.lockscreen_notifications =
            gs_bool("org.gnome.desktop.notifications", "show-in-lock-screen").unwrap_or(true);
        info.night_light = gs_bool(
            "org.gnome.settings-daemon.plugins.color",
            "night-light-enabled",
        )
        .unwrap_or(false);
        info.night_light_auto = gs_bool(
            "org.gnome.settings-daemon.plugins.color",
            "night-light-schedule-automatic",
        )
        .unwrap_or(true);
        info.night_light_from = gsettings_text(
            "org.gnome.settings-daemon.plugins.color",
            "night-light-schedule-from",
        )
        .parse::<f32>()
        .unwrap_or(20.0);
        info.night_light_to = gsettings_text(
            "org.gnome.settings-daemon.plugins.color",
            "night-light-schedule-to",
        )
        .parse::<f32>()
        .unwrap_or(6.0);
        info.night_light_temp = gs_uint(
            "org.gnome.settings-daemon.plugins.color",
            "night-light-temperature",
        )
        .unwrap_or(2700);
        // Migration for 0.5.4 and earlier: the page showed "Sunset to sunrise"
        // off but left GNOME's manual 20:00-06:00 window in place, so the
        // night light switch did nothing during the day. Treat that default
        // as "no schedule" once, then let the user opt in again.
        let marker = format!(
            "{}/.config/win11-settings/night-light-migrated",
            if info.home.is_empty() {
                std::env::var("HOME").unwrap_or_default()
            } else {
                info.home.clone()
            }
        );
        if !std::path::Path::new(&marker).exists() {
            if !info.night_light_auto
                && (info.night_light_from - 20.0).abs() < 0.01
                && (info.night_light_to - 6.0).abs() < 0.01
            {
                let _ = set_night_light_schedule(false, 0.0, 24.0);
                info.night_light_from = 0.0;
                info.night_light_to = 24.0;
            }
            if let Some(dir) = std::path::Path::new(&marker).parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&marker, "1\n");
        }
        info.night_light_scheduled = info.night_light_auto
            || !night_light_all_day(info.night_light_from, info.night_light_to);
        info.idle_delay = gs_uint("org.gnome.desktop.session", "idle-delay").unwrap_or(300);

        info.clock_show_date =
            gs_bool("org.gnome.desktop.interface", "clock-show-date").unwrap_or(true);
        info.clock_show_weekday =
            gs_bool("org.gnome.desktop.interface", "clock-show-weekday").unwrap_or(false);
        info.clock_show_seconds =
            gs_bool("org.gnome.desktop.interface", "clock-show-seconds").unwrap_or(false);
        info.battery_percentage =
            gs_bool("org.gnome.desktop.interface", "show-battery-percentage").unwrap_or(true);
        info.cursor_size = gs_uint("org.gnome.desktop.interface", "cursor-size").unwrap_or(24);

        info.mouse_speed = gsettings_text("org.gnome.desktop.peripherals.mouse", "speed")
            .parse::<f32>()
            .unwrap_or(0.0);
        info.mouse_natural =
            gs_bool("org.gnome.desktop.peripherals.mouse", "natural-scroll").unwrap_or(false);
        info.mouse_left_handed =
            gs_bool("org.gnome.desktop.peripherals.mouse", "left-handed").unwrap_or(false);
        info.touchpad_tap =
            gs_bool("org.gnome.desktop.peripherals.touchpad", "tap-to-click").unwrap_or(true);
        info.touchpad_natural =
            gs_bool("org.gnome.desktop.peripherals.touchpad", "natural-scroll").unwrap_or(false);
        info.touchpad_disable_typing = gs_bool(
            "org.gnome.desktop.peripherals.touchpad",
            "disable-while-typing",
        )
        .unwrap_or(true);
        info.touchpad_two_finger = gs_bool(
            "org.gnome.desktop.peripherals.touchpad",
            "two-finger-scrolling-enabled",
        )
        .unwrap_or(true);
        info.touchpad_speed = gsettings_text("org.gnome.desktop.peripherals.touchpad", "speed")
            .parse::<f32>()
            .unwrap_or(0.0);

        info.key_repeat =
            gs_bool("org.gnome.desktop.peripherals.keyboard", "repeat").unwrap_or(true);
        info.repeat_delay =
            gs_uint("org.gnome.desktop.peripherals.keyboard", "delay").unwrap_or(500);
        info.repeat_interval =
            gs_uint("org.gnome.desktop.peripherals.keyboard", "repeat-interval").unwrap_or(30);
        info.input_sources = input_sources();

        info.sleep_ac = gsettings_text(
            "org.gnome.settings-daemon.plugins.power",
            "sleep-inactive-ac-type",
        )
        .contains("suspend");
        info.sleep_battery = gsettings_text(
            "org.gnome.settings-daemon.plugins.power",
            "sleep-inactive-battery-type",
        )
        .contains("suspend");
        info.lid_ac = gsettings_text(
            "org.gnome.settings-daemon.plugins.power",
            "lid-close-ac-action",
        )
        .trim_matches('\'')
        .to_owned();
        info.lid_battery = gsettings_text(
            "org.gnome.settings-daemon.plugins.power",
            "lid-close-battery-action",
        )
        .trim_matches('\'')
        .to_owned();
        info.power_button = gsettings_text(
            "org.gnome.settings-daemon.plugins.power",
            "power-button-action",
        )
        .trim_matches('\'')
        .to_owned();
        if info.power_button.is_empty() {
            info.power_button = "suspend".to_owned();
        }
        info.low_battery_saver = gs_bool(
            "org.gnome.settings-daemon.plugins.power",
            "power-saver-profile-on-low-battery",
        )
        .unwrap_or(false);

        let (microphone_volume, input_muted) = input_state();
        info.microphone_volume = microphone_volume;
        info.input_muted = input_muted;
        info.sources = sources();
        info.default_source = default_source_name();
        if info.default_source.is_empty() {
            info.default_source = info
                .sources
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_default();
        }

        info.automatic_updates = unattended_upgrades_enabled();

        info.browsers = handlers_with("WebBrowser");
        info.mail_clients = handlers_with("Email");
        info.editors = handlers_with("TextEditor");
        info.default_browser = xdg_settings_get("default-web-browser");
        info.default_mail = xdg_mime_query("x-scheme-handler/mailto");
        info.default_editor = xdg_mime_query("text/plain");
        info.mime_defaults = mime_defaults();

        let (battery, charging) = battery_state();
        info.battery = battery;
        info.charging = charging;
        info.power_profile = sh("powerprofilesctl get");
        if info.power_profile.is_empty() {
            info.power_profile = "balanced".to_owned();
        }

        info.clock_24h = gsettings_text("org.gnome.desktop.interface", "clock-format")
            .trim_matches('\'')
            .eq_ignore_ascii_case("24h");
        // Both clock values come from a single timedatectl call.
        let clock = sh("timedatectl show -p Timezone -p NTP --value");
        let mut clock_lines = clock.lines();
        info.timezone = clock_lines.next().unwrap_or("").trim().to_owned();
        info.ntp_enabled = clock_lines.next().unwrap_or("").trim() == "yes";
        info.locale = std::env::var("LANG").unwrap_or_else(|_| {
            sh("locale")
                .lines()
                .find(|l| l.starts_with("LANG="))
                .map(|l| l.trim_start_matches("LANG=").to_owned())
                .unwrap_or_default()
        });
        info.local_time = sh("date '+%H:%M, %A %d %B %Y'");
        info.wallpaper = gsettings_text("org.gnome.desktop.background", "picture-uri")
            .trim_matches('\'')
            .to_owned();
        info.autologin = read_autologin();

        let bluetooth = sh("bluetoothctl show");
        info.bluetooth_on = bluetooth.lines().any(|l| l.trim() == "Powered: yes");
        info.discoverable = bluetooth.lines().any(|l| l.trim() == "Discoverable: yes");
        if info.bluetooth_on {
            info.bluetooth_devices = bluetooth_devices();
        }
        info.net_devices = net_devices();
        info.wifi_enabled = sh("nmcli radio wifi 2>/dev/null").trim() == "enabled";
        info.wifi_networks = if info.wifi_enabled {
            wifi_networks(false)
        } else {
            Vec::new()
        };

        info.proxy_mode = gsettings_text("org.gnome.system.proxy", "mode")
            .trim_matches('\'')
            .to_owned();
        info.proxy_http_host = gsettings_text("org.gnome.system.proxy.http", "host")
            .trim_matches('\'')
            .to_owned();
        info.proxy_http_port = gs_uint("org.gnome.system.proxy.http", "port").unwrap_or(8080);
        info.proxy_socks_host = gsettings_text("org.gnome.system.proxy.socks", "host")
            .trim_matches('\'')
            .to_owned();
        info.proxy_socks_port = gs_uint("org.gnome.system.proxy.socks", "port").unwrap_or(1080);
        info.proxy_ignore = gsettings_text("org.gnome.system.proxy", "ignore-hosts")
            .trim()
            .trim_start_matches("@as")
            .trim()
            .trim_matches(['[', ']'])
            .replace(['\'', '"'], "")
            .to_owned();

        info.wine = {
            let v = sh("wine --version");
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        };
        info.waydroid = command_exists("waydroid");
        info.steam =
            command_exists("steam") || Path::new(&format!("{}/.steam", info.home)).exists();

        save_cache(&info);
        info
    }

    pub fn device_title(&self) -> String {
        if !self.product.is_empty() {
            if !self.vendor.is_empty() {
                format!("{} {}", self.vendor, self.product)
            } else {
                self.product.clone()
            }
        } else {
            format!("{} ({})", self.hostname, std::env::consts::ARCH)
        }
    }

    pub fn battery_text(&self) -> String {
        match self.battery {
            Some(pct) => {
                let state = if self.charging {
                    "charging"
                } else {
                    "on battery"
                };
                format!("{}% · {}", pct, state)
            }
            None => "No battery detected".to_owned(),
        }
    }

    pub fn volume_text(&self) -> String {
        if self.muted {
            "Muted".to_owned()
        } else {
            format!("{}% · {}", self.volume, self.sink_label())
        }
    }

    pub fn sink_label(&self) -> String {
        self.sinks
            .iter()
            .find(|s| s.name == self.default_sink)
            .map(|s| s.description.clone())
            .or_else(|| self.sinks.first().map(|s| s.description.clone()))
            .unwrap_or_else(|| "Audio output".to_owned())
    }

    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Device: {}\n", self.hostname));
        s.push_str(&format!("OS: {}\n", self.os_pretty));
        s.push_str(&format!("Kernel: {}\n", self.kernel));
        s.push_str(&format!("Desktop: {}\n", self.desktop));
        s.push_str(&format!("Processor: {}\n", self.cpu));
        s.push_str(&format!("Memory: {:.1} GB\n", self.mem_total_gb));
        s.push_str(&format!(
            "Storage: {:.0} GB total, {:.0} GB free\n",
            self.disk_total_gb, self.disk_free_gb
        ));
        s.push_str(&format!("Uptime: {}\n", self.uptime));
        s
    }
}

// ---------------------------------------------------------------- commands

pub fn sh(cmd: &str) -> String {
    let out = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        Err(_) => String::new(),
    }
}

pub fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", name))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn read_trim(path: &str) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

fn getent_full_name(user: &str) -> String {
    let line = sh(&format!("getent passwd {}", shell_quote(user)));
    line.split(':')
        .nth(4)
        .unwrap_or("")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn os_field(os: &str, key: &str) -> String {
    os.lines()
        .find_map(|l| l.strip_prefix(&format!("{}=", key)))
        .map(|v| v.trim_matches('"').to_owned())
        .unwrap_or_default()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn human_desktop(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "gnome" => "GNOME".to_owned(),
        "ubuntu:gnome" => "Ubuntu (GNOME)".to_owned(),
        "kde" => "KDE Plasma".to_owned(),
        "xfce" => "Xfce".to_owned(),
        "unity" => "Unity".to_owned(),
        _ => raw.replace(':', " · "),
    }
}

fn cpu_model() -> String {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let raw = cpuinfo
        .lines()
        .find_map(|l| {
            l.strip_prefix("model name")
                .map(|v| v.trim_start_matches([':', ' ']).to_owned())
        })
        .or_else(|| {
            cpuinfo.lines().find_map(|l| {
                l.strip_prefix("Hardware")
                    .map(|v| v.trim_start_matches([':', ' ']).to_owned())
            })
        })
        .unwrap_or_default();
    raw.replace("(R)", "")
        .replace("(TM)", "")
        .replace("  ", " ")
        .trim()
        .to_owned()
}

fn memory() -> Option<(f32, f32)> {
    let mem = fs::read_to_string("/proc/meminfo").ok()?;
    let field = |name: &str| -> f32 {
        mem.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f32>().ok())
            .map(|kb| kb / 1024.0 / 1024.0)
            .unwrap_or(0.0)
    };
    Some((field("MemTotal:"), field("MemAvailable:")))
}

fn root_disk() -> (f32, f32, f32) {
    let out = sh("df -B1 / | tail -1");
    let mut it = out.split_whitespace();
    let total = it.nth(1).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let used = it.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let avail = it.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    if total <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let gb = 1024.0f64.powi(3);
    (
        (total / gb) as f32,
        (avail / gb) as f32,
        (used / total * 100.0) as f32,
    )
}

/// Slow by nature (walks the whole home directory): never called from `load`.
pub fn home_usage_gb(home: &str) -> Option<f32> {
    if home.is_empty() {
        return None;
    }
    let out = sh(&format!(
        "timeout 20 du -sx -B1 {} 2>/dev/null | head -1",
        shell_quote(home)
    ));
    let bytes = out.split_whitespace().next()?.parse::<f64>().ok()?;
    Some((bytes / 1024.0f64.powi(3)) as f32)
}

fn mounts() -> Vec<Mount> {
    let out =
        sh("df -B1 -x tmpfs -x devtmpfs -x squashfs -x overlay -x efivarfs -x ramfs 2>/dev/null");
    let mut result = Vec::new();
    for line in out.lines().skip(1) {
        let cols = line.split_whitespace().collect::<Vec<_>>();
        if cols.len() < 6 {
            continue;
        }
        // mount point is the remainder joined (paths with spaces are rare but possible)
        let path = cols[5..].join(" ");
        if !(path == "/"
            || path == "/home"
            || path == "/boot"
            || path == "/var"
            || path.starts_with("/media/")
            || path.starts_with("/mnt/"))
        {
            continue;
        }
        let total = cols[1].parse::<f64>().unwrap_or(0.0);
        let used = cols[2].parse::<f64>().unwrap_or(0.0);
        let _free = cols[3].parse::<f64>().unwrap_or(0.0);
        if total <= 0.0 {
            continue;
        }
        let gb = 1024.0f64.powi(3);
        result.push(Mount {
            path,
            used_gb: (used / gb) as f32,
            total_gb: (total / gb) as f32,
        });
    }
    result.truncate(6);
    result
}

/// Talks to the compositor's DisplayConfig API, which is what GNOME Settings
/// itself uses. On Wayland `xrandr` cannot change anything, so this is the only
/// way to make resolution, refresh rate and scale actually apply.
const MUTTER_SCRIPT: &str = r#"
import json, os, re, sys
from gi.repository import Gio, GLib

DEST = "org.gnome.Mutter.DisplayConfig"
PATH = "/org/gnome/Mutter/DisplayConfig"
IFACE = "org.gnome.Mutter.DisplayConfig"
BUS = Gio.bus_get_sync(Gio.BusType.SESSION, None)


def state():
    return BUS.call_sync(DEST, PATH, IFACE, "GetCurrentState", None, None,
                         Gio.DBusCallFlags.NONE, -1, None).unpack()


def xml_mode(connector):
    """Fallback current mode for compositors without the is-current flag."""
    try:
        text = open(os.path.expanduser("~/.config/monitors.xml")).read()
    except OSError:
        return None
    for block in text.split("<monitor>")[1:]:
        if "<connector>" + connector + "</connector>" not in block:
            continue
        def tag(name, block=block):
            m = re.search("<" + name + ">([0-9.]+)</" + name + ">", block)
            return float(m.group(1)) if m else None
        w, h, r = tag("width"), tag("height"), tag("rate")
        if w and h and r:
            return (int(w), int(h), r)
    return None


def describe(st):
    _serial, monitors, logical, _props = st
    scale_of = {}
    for (_x, _y, scale, _t, _p, mons, _mp) in logical:
        for (conn, _v, _p, _s) in mons:
            scale_of[conn] = scale
    out = []
    for (spec, modes, _mp) in monitors:
        conn = spec[0]
        fallback = xml_mode(conn)
        current = None
        for (mid, _w, _h, _r, _ps, _scales, mp) in modes:
            if mp.get("is-current"):
                current = mid
        if current is None and fallback is not None:
            for (mid, w, h, r, _ps, _scales, _mp) in modes:
                if (w, h) == fallback[:2] and abs(r - fallback[2]) < 0.1:
                    current = mid
                    break
        entries = []
        for (mid, w, h, rate, _ps, scales, _mp) in modes:
            entries.append({
                "id": mid, "width": w, "height": h, "rate": round(rate, 3),
                "scales": [round(sc, 3) for sc in scales],
                "current": mid == current,
            })
        entries.sort(key=lambda m: (m["width"] * m["height"], m["rate"]), reverse=True)
        out.append({
            "connector": conn,
            "scale": scale_of.get(conn, 1.0),
            "current": current or "",
            "modes": entries,
        })
    return out


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "no command"}))
        return
    cmd = sys.argv[1]
    try:
        st = state()
    except Exception as exc:  # noqa: BLE001 - surface the reason to the caller
        print(json.dumps({"error": str(exc)}))
        return
    if cmd == "list":
        print(json.dumps({"monitors": describe(st)}, separators=(",", ":")))
        return
    if cmd == "set":
        if len(sys.argv) < 4:
            print(json.dumps({"error": "usage"}))
            return
        connector, mode_id = sys.argv[2], sys.argv[3]
        scale = float(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[4] else None
        serial, monitors, logical, props = st
        current = {}
        for (spec, modes, _mp) in monitors:
            for (mid, _w, _h, _r, _ps, _scales, mp) in modes:
                if mp.get("is-current"):
                    current[spec[0]] = mid
        wanted = None
        for (spec, modes, _mp) in monitors:
            if spec[0] != connector:
                continue
            for (mid, _w, _h, _r, _ps, scales, _mp) in modes:
                if mid == mode_id:
                    wanted = (mid, [round(sc, 3) for sc in scales])
        if wanted is None:
            print(json.dumps({"error": "unknown mode"}))
            return
        if scale is not None and wanted[1] and round(scale, 3) not in wanted[1]:
            print(json.dumps({"error": "unsupported scale"}))
            return
        new_logical = []
        for (x, y, cur_scale, transform, primary, mons, _mp) in logical:
            new_mons = []
            for (conn, _v, _p, _s) in mons:
                new_mons.append((conn, wanted[0] if conn == connector else current.get(conn, ""), {}))
            new_scale = scale if (scale is not None and any(c == connector for (c, _v, _p, _s) in mons)) else cur_scale
            new_logical.append((x, y, new_scale, transform, primary, new_mons))
        gprops = {}
        if "layout-mode" in props:
            gprops["layout-mode"] = GLib.Variant("u", props["layout-mode"])
        BUS.call_sync(DEST, PATH, IFACE, "ApplyMonitorsConfig",
                      GLib.Variant("(uua(iiduba(ssa{sv}))a{sv})", (serial, 2, new_logical, gprops)),
                      None, Gio.DBusCallFlags.NONE, -1, None)
        print(json.dumps({"ok": True}))
        return
    print(json.dumps({"error": "unknown command"}))


main()
"#;

fn mutter_python(args: &[&str]) -> Option<String> {
    if !command_exists("python3") {
        return None;
    }
    let out = Command::new("python3")
        .arg("-c")
        .arg(MUTTER_SCRIPT)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Read the compositor's real mode list; falls back to parsing `xrandr` when
/// the DisplayConfig service is unavailable (X11 without Mutter).
fn display_monitors() -> Vec<DisplayMonitor> {
    if let Some(raw) = mutter_python(&["list"]) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            let mut monitors = Vec::new();
            for mon in value
                .get("monitors")
                .and_then(|m| m.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[])
            {
                let connector = mon
                    .get("connector")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                if connector.is_empty() {
                    continue;
                }
                let mut modes = Vec::new();
                for m in mon
                    .get("modes")
                    .and_then(|v| v.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[])
                {
                    modes.push(DisplayMode {
                        id: m
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_owned(),
                        width: m.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        height: m.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        rate: m.get("rate").and_then(|v| v.as_f64()).unwrap_or(60.0) as f32,
                        scales: m
                            .get("scales")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|s| s.as_f64())
                                    .map(|s| s as f32)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        current: m.get("current").and_then(|v| v.as_bool()).unwrap_or(false),
                    });
                }
                if modes.is_empty() {
                    continue;
                }
                monitors.push(DisplayMonitor {
                    connector,
                    scale: mon.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    modes,
                });
            }
            if !monitors.is_empty() {
                return monitors;
            }
        }
    }
    display_monitors_xrandr()
}

fn display_monitors_xrandr() -> Vec<DisplayMonitor> {
    let out = sh("xrandr --current 2>/dev/null");
    let mut monitors: Vec<DisplayMonitor> = Vec::new();
    for line in out.lines() {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let fields: Vec<&str> = line.split_whitespace().collect();
        if !indented {
            if fields.len() < 2 || !fields[1].starts_with("connected") {
                continue;
            }
            monitors.push(DisplayMonitor {
                connector: fields[0].to_owned(),
                scale: 1.0,
                modes: Vec::new(),
            });
            continue;
        }
        let Some(monitor) = monitors.last_mut() else {
            continue;
        };
        let Some(mode) = fields.first() else { continue };
        let (Some(w), Some(h)) = (
            mode.split('x').next().and_then(|v| v.parse::<u32>().ok()),
            mode.split('x').nth(1).and_then(|v| v.parse::<u32>().ok()),
        ) else {
            continue;
        };
        for token in &fields[1..] {
            let current = token.contains('*');
            let hz = token.trim_end_matches(['*', '+']);
            let Ok(rate) = hz.parse::<f32>() else {
                continue;
            };
            monitor.modes.push(DisplayMode {
                id: (*mode).to_owned(),
                width: w,
                height: h,
                rate,
                scales: vec![1.0],
                current,
            });
        }
    }
    monitors
}

/// Current resolution and refresh rate, taken from whichever display model
/// reports the active mode.
fn display_mode(monitors: &[DisplayMonitor]) -> (String, String) {
    for monitor in monitors {
        if let Some(mode) = monitor.modes.iter().find(|m| m.current) {
            return (
                format!("{} × {}", mode.width, mode.height),
                format!("{:.0} Hz", mode.rate),
            );
        }
    }
    ("Unknown".to_owned(), "—".to_owned())
}

/// Apply a mode (and optionally a fractional scale) through the compositor.
pub fn set_display_mode(connector: &str, mode_id: &str, scale: f32) -> Result<(), String> {
    let scale = format!("{scale}");
    match mutter_python(&["set", connector, mode_id, &scale]) {
        Some(raw) => {
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| format!("unexpected reply: {e}"))?;
            if value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                Ok(())
            } else {
                Err(value
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("the display server refused the change")
                    .to_owned())
            }
        }
        None => Err("display settings need python3-gi and a running desktop session".to_owned()),
    }
}

fn text_scale() -> String {
    let raw = gsettings_text("org.gnome.desktop.interface", "text-scaling-factor");
    let factor = raw.parse::<f32>().unwrap_or(1.0);
    format!("{:.0}%", factor * 100.0)
}

fn brightness() -> Option<u8> {
    let entries = fs::read_dir("/sys/class/backlight").ok()?;
    for entry in entries.flatten() {
        let dir = entry.path();
        let cur = fs::read_to_string(dir.join("brightness"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        let max = fs::read_to_string(dir.join("max_brightness"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        if max > 0.0 {
            return Some((cur / max * 100.0).round() as u8);
        }
    }
    None
}

fn audio_state() -> (u32, bool) {
    if command_exists("pactl") {
        let vol = sh("pactl get-sink-volume @DEFAULT_SINK@ 2>/dev/null");
        let volume = vol
            .split('%')
            .next()
            .and_then(|s| s.split_whitespace().last())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(50);
        let muted = sh("pactl get-sink-mute @DEFAULT_SINK@ 2>/dev/null").contains("yes");
        return (volume, muted);
    }
    // PipeWire's own control tool. "Volume: 0.65 [MUTED]"
    let out = sh("wpctl get-volume @DEFAULT_AUDIO_SINK@ 2>/dev/null");
    let volume = out
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v * 100.0).round() as u32)
        .unwrap_or(50);
    let muted = out.contains("[MUTED]");
    (volume, muted)
}

fn default_sink_name() -> String {
    if command_exists("pactl") {
        let name = sh("pactl get-default-sink");
        if !name.is_empty() {
            return name;
        }
    }
    wpctl_property("@DEFAULT_AUDIO_SINK@", "node.name")
}

fn wpctl_property(target: &str, key: &str) -> String {
    let out = sh(&format!(
        "wpctl inspect {} 2>/dev/null",
        shell_quote(target)
    ));
    out.lines()
        .find_map(|line| line.split_once('=').filter(|(k, _)| k.contains(key)))
        .map(|(_, v)| v.trim().trim_matches('"').to_owned())
        .unwrap_or_default()
}

fn sinks() -> Vec<Sink> {
    if command_exists("pactl") {
        let out = sh("pactl list sinks 2>/dev/null");
        let mut sinks = Vec::new();
        let mut name = String::new();
        let mut desc = String::new();
        for line in out.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Name: ") {
                name = rest.to_owned();
                desc.clear();
            } else if let Some(rest) = line.strip_prefix("Description: ") {
                desc = rest.to_owned();
                if !name.is_empty() {
                    sinks.push(Sink {
                        name: name.clone(),
                        description: desc.clone(),
                        id: String::new(),
                    });
                }
            }
        }
        if !sinks.is_empty() {
            return sinks;
        }
    }
    wpctl_sinks()
}

/// Parse the "Sinks:" block of `wpctl status`.
fn wpctl_sinks() -> Vec<Sink> {
    let out = sh("wpctl status 2>/dev/null");
    let mut sinks = Vec::new();
    let mut in_sinks = false;
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("Sinks:") {
            in_sinks = true;
            continue;
        }
        if !in_sinks {
            continue;
        }
        let rest = trimmed
            .rsplit('│')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .trim_start_matches('*')
            .trim();
        if rest.is_empty() {
            continue;
        }
        let Some((id, tail)) = rest.split_once('.') else {
            break;
        };
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            // End of the Sinks block (Sources:, Filters:, ...).
            break;
        }
        let description = tail.split('[').next().unwrap_or(tail).trim().to_owned();
        let name = wpctl_property(id, "node.name");
        sinks.push(Sink {
            name: if name.is_empty() { id.to_owned() } else { name },
            description: if description.is_empty() {
                format!("Output {id}")
            } else {
                description
            },
            id: id.to_owned(),
        });
    }
    sinks
}

/// Parse `gsettings get org.gnome.desktop.input-sources sources`.
/// Returns the layout ids, e.g. ["us", "de"] (xkb sources only).
fn input_sources() -> Vec<String> {
    let raw = gsettings_text("org.gnome.desktop.input-sources", "sources");
    let mut ids = Vec::new();
    let mut rest = raw.as_str();
    while let Some(pos) = rest.find("('xkb'") {
        let after = rest[pos + "('xkb'".len()..].trim_start();
        let Some(after) = after.strip_prefix(',') else {
            break;
        };
        let Some(after) = after.trim_start().strip_prefix('\'') else {
            break;
        };
        let Some(end) = after.find('\'') else { break };
        let id = after[..end].trim();
        if !id.is_empty() {
            ids.push(id.to_owned());
        }
        rest = &after[end + 1..];
    }
    ids
}

/// Set the xkb input sources from layout ids (e.g. ["us", "de"]).
pub fn set_input_sources(ids: &[String]) -> bool {
    if ids.is_empty() {
        return false;
    }
    let tuple = ids
        .iter()
        .map(|id| format!("('xkb', '{}')", id.replace('\'', "")))
        .collect::<Vec<_>>()
        .join(", ");
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.input-sources",
            "sources",
            &format!("[{tuple}]"),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Common XKB layouts offered when adding an input source.
pub const KB_LAYOUTS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("br", "Portuguese (Brazil)"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("pl", "Polish"),
    ("nl", "Dutch"),
    ("be", "Belgian"),
    ("ch", "Swiss"),
    ("at", "Austrian"),
    ("se", "Swedish"),
    ("no", "Norwegian"),
    ("dk", "Danish"),
    ("fi", "Finnish"),
    ("cz", "Czech"),
    ("hu", "Hungarian"),
    ("ro", "Romanian"),
    ("gr", "Greek"),
    ("tr", "Turkish"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
    ("cn", "Chinese"),
    ("in", "India"),
    ("ara", "Arabic"),
    ("il", "Hebrew"),
    ("my", "Malay"),
    ("th", "Thai"),
    ("vn", "Vietnamese"),
    ("et", "Estonian"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("sk", "Slovak"),
    ("si", "Slovenian"),
    ("hr", "Croatian"),
    ("bg", "Bulgarian"),
    ("rs", "Serbian"),
];

fn input_state() -> (u32, bool) {
    if command_exists("pactl") {
        let vol = sh("pactl get-source-volume @DEFAULT_SOURCE@ 2>/dev/null");
        let volume = vol
            .split('%')
            .next()
            .and_then(|s| s.split_whitespace().last())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(70);
        let muted = sh("pactl get-source-mute @DEFAULT_SOURCE@ 2>/dev/null").contains("yes");
        return (volume, muted);
    }
    let out = sh("wpctl get-volume @DEFAULT_AUDIO_SOURCE@ 2>/dev/null");
    let volume = out
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v * 100.0).round() as u32)
        .unwrap_or(70);
    (volume, out.contains("[MUTED]"))
}

fn default_source_name() -> String {
    if command_exists("pactl") {
        let name = sh("pactl get-default-source");
        if !name.is_empty() {
            return name;
        }
    }
    wpctl_property("@DEFAULT_AUDIO_SOURCE@", "node.name")
}

fn sources() -> Vec<Sink> {
    if command_exists("pactl") {
        let out = sh("pactl list sources 2>/dev/null");
        let mut sinks = Vec::new();
        let mut name = String::new();
        let mut desc = String::new();
        for line in out.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Name: ") {
                name = rest.to_owned();
                desc.clear();
            } else if let Some(rest) = line.strip_prefix("Description: ") {
                desc = rest.to_owned();
                // Skip monitor sources — they are not real microphones.
                if !name.is_empty() && !name.ends_with(".monitor") {
                    sinks.push(Sink {
                        name: name.clone(),
                        description: desc.clone(),
                        id: String::new(),
                    });
                }
            }
        }
        if !sinks.is_empty() {
            return sinks;
        }
    }
    wpctl_sources()
}

/// Parse the "Sources:" block of `wpctl status`.
fn wpctl_sources() -> Vec<Sink> {
    let out = sh("wpctl status 2>/dev/null");
    let mut sinks = Vec::new();
    let mut in_sources = false;
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("Sources:") {
            in_sources = true;
            continue;
        }
        if !in_sources {
            continue;
        }
        let rest = trimmed
            .rsplit('│')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .trim_start_matches('*')
            .trim();
        if rest.is_empty() {
            continue;
        }
        let Some((id, tail)) = rest.split_once('.') else {
            break;
        };
        let id = id.trim();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        let description = tail.split('[').next().unwrap_or(tail).trim().to_owned();
        let name = wpctl_property(id, "node.name");
        sinks.push(Sink {
            name: if name.is_empty() { id.to_owned() } else { name },
            description: if description.is_empty() {
                format!("Input {id}")
            } else {
                description
            },
            id: id.to_owned(),
        });
    }
    sinks
}

/// Whether unattended (automatic) security upgrades are enabled.
/// Reads the apt config; `None` when unattended-upgrades is not installed.
fn unattended_upgrades_enabled() -> Option<bool> {
    if !Path::new("/usr/bin/unattended-upgrade").exists() {
        return None;
    }
    let mut files = vec!["/etc/apt/apt.conf.d/20auto-upgrades".to_owned()];
    if let Ok(entries) = fs::read_dir("/etc/apt/apt.conf.d") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.contains("auto-upgrades") {
                    files.push(entry.path().to_string_lossy().into_owned());
                }
            }
        }
    }
    let mut found = false;
    let mut enabled = false;
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("APT::Periodic::Unattended-Upgrade") {
                if let Some(value) = rest.trim_matches([':', ' ', '"']).split_whitespace().next() {
                    found = true;
                    enabled = value.starts_with('1');
                }
            }
        }
    }
    Some(found && enabled)
}

/// Set unattended security upgrades on/off through PolicyKit.
pub fn set_unattended_upgrades(on: bool) -> bool {
    let value = if on { "1" } else { "0" };
    let content = format!(
        "APT::Periodic::Update-Package-Lists \"1\";\nAPT::Periodic::Unattended-Upgrade \"{value}\";\n"
    );
    let out = Command::new("pkexec")
        .arg("sh")
        .arg("-c")
        .arg(format!(
            "printf '%s' \"$1\" > /etc/apt/apt.conf.d/20auto-upgrades",
        ))
        .arg("_")
        .arg(&content)
        .output();
    matches!(out, Ok(o) if o.status.success())
}

// Desktop entries are scanned once per load (below) and shared by all categories.
thread_local! {
    /// Desktop entries (id, name, categories), scanned once per load so the
    /// three "default app" categories share a single directory walk.
    static DESKTOP_ENTRIES: std::cell::RefCell<Option<Vec<(String, String, String, String)>>> =
        const { std::cell::RefCell::new(None) };
}

fn desktop_entries() -> Vec<(String, String, String, String)> {
    if let Some(hit) = DESKTOP_ENTRIES.with(|c| c.borrow().clone()) {
        return hit;
    }
    let mut roots = vec![
        "/usr/share/applications".to_owned(),
        "/var/lib/snapd/desktop/applications".to_owned(),
        "/var/lib/flatpak/exports/share/applications".to_owned(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(format!("{home}/.local/share/applications"));
        roots.push(format!(
            "{home}/.local/share/flatpak/exports/share/applications"
        ));
    }
    let mut found: Vec<(String, String, String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let desktop_id = entry.file_name().to_string_lossy().into_owned();
            if !seen.insert(desktop_id.clone()) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            // Read only the [Desktop Entry] group (skip actions such as "New Window").
            let mut in_entry = false;
            let mut no_display = false;
            let mut name: Option<String> = None;
            let mut categories = String::new();
            let mut mimes = String::new();
            for raw in text.lines() {
                let line = raw.trim();
                if line.starts_with('[') {
                    in_entry = line == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(v) = line.strip_prefix("NoDisplay=") {
                    no_display = v.trim().eq_ignore_ascii_case("true");
                } else if let Some(v) = line.strip_prefix("Name=") {
                    if name.is_none() {
                        name = Some(v.trim().to_owned());
                    }
                } else if let Some(v) = line.strip_prefix("Categories=") {
                    categories = v.to_owned();
                } else if let Some(v) = line.strip_prefix("MimeType=") {
                    mimes = v.to_owned();
                }
            }
            if no_display {
                continue;
            }
            let Some(name) = name else { continue };
            found.push((desktop_id, name, categories, mimes));
        }
    }
    DESKTOP_ENTRIES.with(|c| *c.borrow_mut() = Some(found.clone()));
    found
}

/// A file-type family whose default application can be changed in one go.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MimeDefault {
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub mimes: Vec<String>,
    pub current: String,
    pub mixed: bool,
    pub apps: Vec<(String, String)>,
}

struct MimeGroup {
    kind: &'static str,
    title: &'static str,
    subtitle: &'static str,
    mimes: &'static [&'static str],
}

const MIME_GROUPS: &[MimeGroup] = &[
    MimeGroup {
        kind: "document",
        title: "Documents",
        subtitle: "Word, OpenDocument and rich text",
        mimes: &[
            "application/msword",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.oasis.opendocument.text",
            "application/rtf",
        ],
    },
    MimeGroup {
        kind: "spreadsheet",
        title: "Spreadsheets",
        subtitle: "Excel, OpenDocument and CSV",
        mimes: &[
            "application/vnd.ms-excel",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.oasis.opendocument.spreadsheet",
            "text/csv",
        ],
    },
    MimeGroup {
        kind: "presentation",
        title: "Presentations",
        subtitle: "PowerPoint and OpenDocument slides",
        mimes: &[
            "application/vnd.ms-powerpoint",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "application/vnd.oasis.opendocument.presentation",
        ],
    },
    MimeGroup {
        kind: "pdf",
        title: "PDF documents",
        subtitle: "Open PDF files",
        mimes: &["application/pdf"],
    },
    MimeGroup {
        kind: "image",
        title: "Photos",
        subtitle: "PNG, JPEG, GIF and WebP",
        mimes: &["image/*"],
    },
    MimeGroup {
        kind: "video",
        title: "Video",
        subtitle: "MP4, MKV and WebM",
        mimes: &["video/*"],
    },
    MimeGroup {
        kind: "audio",
        title: "Music",
        subtitle: "MP3, FLAC, OGG and WAV",
        mimes: &["audio/*"],
    },
    MimeGroup {
        kind: "archive",
        title: "Archives",
        subtitle: "ZIP, 7z, RAR and tar",
        mimes: &[
            "application/zip",
            "application/x-7z-compressed",
            "application/vnd.rar",
            "application/x-tar",
            "application/gzip",
            "application/x-xz",
        ],
    },
    MimeGroup {
        kind: "calendar",
        title: "Calendar",
        subtitle: "ICS calendar files",
        mimes: &["text/calendar"],
    },
];

/// Does a desktop entry's MimeType list cover any of the wanted types?
fn declares_mime(declared: &str, mimes: &[String]) -> bool {
    declared
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .any(|d| {
            mimes.iter().any(|m| match m.strip_suffix("/*") {
                // "image/*" in a group matches "image/png", "image/jpeg", ...
                Some(prefix) => d.starts_with(&format!("{prefix}/")),
                None => m == d,
            })
        })
}

/// Build the default-application rows for the common file families.
fn mime_defaults() -> Vec<MimeDefault> {
    let entries = desktop_entries();
    MIME_GROUPS
        .iter()
        .map(|group| {
            let mimes: Vec<String> = group.mimes.iter().map(|m| (*m).to_owned()).collect();
            let mut apps: Vec<(String, String)> = entries
                .iter()
                .filter(|(_, _, _, declared)| declares_mime(declared, &mimes))
                .map(|(id, name, _, _)| (id.clone(), name.clone()))
                .collect();
            apps.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
            apps.dedup_by(|a, b| a.0 == b.0);

            let queried: Vec<String> = group.mimes.iter().map(|m| xdg_mime_query(m)).collect();
            let current = queried
                .iter()
                .find(|q| !q.is_empty())
                .cloned()
                .unwrap_or_default();
            let mixed = queried
                .iter()
                .filter(|q| !q.is_empty())
                .any(|q| *q != current);
            if !current.is_empty() && !apps.iter().any(|(id, _)| *id == current) {
                let name = entries
                    .iter()
                    .find(|(id, _, _, _)| *id == current)
                    .map(|(_, name, _, _)| name.clone())
                    .unwrap_or_else(|| current.clone());
                apps.insert(0, (current.clone(), name));
            }
            MimeDefault {
                kind: group.kind.to_owned(),
                title: group.title.to_owned(),
                subtitle: group.subtitle.to_owned(),
                mimes,
                current,
                mixed,
                apps,
            }
        })
        .collect()
}

fn handlers_with(category: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = desktop_entries()
        .into_iter()
        .filter(|(_, _, categories, _)| {
            categories
                .split(';')
                .any(|c| c.trim().eq_ignore_ascii_case(category))
        })
        .map(|(id, name, _, _)| (id, name))
        .collect();
    found.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    found
}

fn xdg_settings_get(key: &str) -> String {
    sh(&format!(
        "xdg-settings get {} 2>/dev/null",
        shell_quote(key)
    ))
}

thread_local! {
    /// MIME defaults parsed from the same files `xdg-mime` reads.
    static XDG_DEFAULTS: std::cell::RefCell<
        Option<std::collections::HashMap<String, String>>,
    > = const { std::cell::RefCell::new(None) };
}

/// Search path order that `xdg-mime` uses, so what we display is exactly what
/// `xdg-mime default` writes.
fn xdg_list(var: &str, fallback: &str) -> Vec<std::path::PathBuf> {
    let value = std::env::var(var)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback.to_owned());
    value
        .split(':')
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

fn xdg_data_dirs() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs = vec![std::path::PathBuf::from(format!("{home}/.local/share"))];
    dirs.extend(xdg_list("XDG_DATA_DIRS", "/usr/local/share:/usr/share"));
    dirs
}

fn xdg_config_dirs() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs = vec![std::path::PathBuf::from(
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| format!("{home}/.config")),
    )];
    dirs.extend(xdg_list("XDG_CONFIG_DIRS", "/etc/xdg"));
    dirs
}

/// `mimeapps.list` variants: the plain file, then per-desktop overrides that
/// take precedence for the current desktop.
fn mimeapps_names() -> Vec<String> {
    let mut names = vec!["mimeapps.list".to_owned()];
    if let Ok(desktops) = std::env::var("XDG_CURRENT_DESKTOP") {
        for desktop in desktops.split(':').filter(|d| !d.is_empty()) {
            names.push(format!("{}-mimeapps.list", desktop.to_lowercase()));
        }
    }
    names
}

fn desktop_file_exists(id: &str) -> bool {
    if id.is_empty() || id == "-" || id.contains('/') {
        return false;
    }
    let mut roots: Vec<std::path::PathBuf> = xdg_data_dirs()
        .into_iter()
        .map(|d| d.join("applications"))
        .collect();
    if let Ok(home) = std::env::var("HOME") {
        roots.push(std::path::PathBuf::from(format!(
            "{home}/.local/share/applications"
        )));
    }
    roots.iter().any(|root| root.join(id).is_file())
}

fn parse_associations(
    path: &Path,
    defaults_only: bool,
    check_exists: bool,
    map: &mut std::collections::HashMap<String, String>,
) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let mut active = !defaults_only;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            active = !defaults_only || line == "[Default Applications]";
            continue;
        }
        if !active {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !key.contains('/') {
            continue;
        }
        let id = value
            .split(';')
            .map(str::trim)
            .find(|v| !v.is_empty() && (!check_exists || desktop_file_exists(v)));
        if let Some(id) = id {
            map.insert(key.to_owned(), id.to_owned());
        }
    }
}

/// Every MIME default, resolved the way `xdg-mime query default` does but
/// without spawning a process per type (that cost ~70 ms each).
fn xdg_defaults() -> std::collections::HashMap<String, String> {
    if let Some(hit) = XDG_DEFAULTS.with(|c| c.borrow().clone()) {
        return hit;
    }
    let mut map = std::collections::HashMap::new();
    let config = xdg_config_dirs();
    let data = xdg_data_dirs();
    let menu_prefix = std::env::var("XDG_MENU_PREFIX").unwrap_or_default();
    let names = mimeapps_names();

    // Lowest priority first so that later files overwrite earlier ones.
    for dir in data.iter().rev() {
        let apps = dir.join("applications");
        for name in ["mimeinfo.cache", "defaults.list"] {
            parse_associations(
                &apps.join(format!("{menu_prefix}{name}")),
                false,
                false,
                &mut map,
            );
            if !menu_prefix.is_empty() {
                parse_associations(&apps.join(name), false, false, &mut map);
            }
        }
    }
    for dir in data.iter().rev() {
        let apps = dir.join("applications");
        for name in &names {
            parse_associations(&apps.join(name), true, true, &mut map);
        }
    }
    for dir in config.iter().skip(1).collect::<Vec<_>>().into_iter().rev() {
        for name in &names {
            parse_associations(&dir.join(name), true, true, &mut map);
        }
    }
    for name in &names {
        parse_associations(&config[0].join(name), true, true, &mut map);
    }
    XDG_DEFAULTS.with(|c| *c.borrow_mut() = Some(map.clone()));
    map
}

fn xdg_default_for(mime: &str) -> String {
    if let Some(hit) = xdg_defaults().get(mime) {
        return hit.clone();
    }
    // Nothing configured: fall back to the first desktop entry that declares
    // the type, which is what xdg-mime's own fallback also ends up doing.
    let wanted = [mime.to_owned()];
    desktop_entries()
        .into_iter()
        .find(|(_, _, _, declared)| declares_mime(declared, &wanted))
        .map(|(id, _, _, _)| id)
        .unwrap_or_default()
}

fn xdg_mime_query(mime: &str) -> String {
    xdg_default_for(mime)
}

/// Every MIME type in the default-apps families and its resolved handler.
/// Used by the `WIN11_SETTINGS_MIME` diagnostic.
pub fn mime_report() -> Vec<(String, String)> {
    MIME_GROUPS
        .iter()
        .flat_map(|group| {
            group
                .mimes
                .iter()
                .map(|mime| ((*mime).to_owned(), xdg_default_for(mime)))
        })
        .collect()
}
/// Set the default web browser (desktop id, e.g. "firefox.desktop").
pub fn set_default_browser(desktop_id: &str) -> bool {
    Command::new("xdg-settings")
        .args(["set", "default-web-browser", desktop_id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Point every MIME type in a family at one application.
pub fn set_default_apps(mimes: &[String], desktop_id: &str) -> bool {
    let mut all = true;
    for mime in mimes {
        all &= set_default_for(mime, desktop_id);
    }
    all
}

/// Set the handler for a MIME type (desktop id).
pub fn set_default_for(mime: &str, desktop_id: &str) -> bool {
    XDG_DEFAULTS.with(|c| *c.borrow_mut() = None);
    Command::new("xdg-mime")
        .args(["default", desktop_id, mime])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The IANA time-zone names known to this system (read once per process).
pub fn list_timezones() -> Vec<String> {
    static TIMEZONES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    TIMEZONES
        .get_or_init(|| {
            let out = sh("timedatectl list-timezones 2>/dev/null");
            if !out.is_empty() {
                return out
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            // Fall back to the zoneinfo tree.
            let mut zones = Vec::new();
            fn walk(dir: &std::path::Path, zones: &mut Vec<String>) {
                let Ok(entries) = fs::read_dir(dir) else {
                    return;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, zones);
                    } else if let Ok(rel) = path.strip_prefix("/usr/share/zoneinfo") {
                        let rel = rel.to_string_lossy().replace('\\', "/");
                        if !rel.is_empty()
                            && !rel.ends_with(".tab")
                            && !rel.ends_with(".list")
                            && !rel.starts_with("posix/")
                            && !rel.starts_with("right/")
                            && rel != "posixrules"
                        {
                            zones.push(rel);
                        }
                    }
                }
            }
            walk(std::path::Path::new("/usr/share/zoneinfo"), &mut zones);
            zones.sort();
            zones.dedup();
            zones
        })
        .clone()
}

/// Current local time in the same format the System probe uses.
pub fn local_time() -> String {
    sh("date '+%H:%M, %A %d %B %Y'")
}

/// Set the system time zone. Requires administrator authorisation via pkexec.
pub fn set_timezone(tz: &str) -> bool {
    if !tz
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '+'))
    {
        return false;
    }
    Command::new("pkexec")
        .args(["timedatectl", "set-timezone", tz])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Delete the active Wi-Fi connection profile (NetworkManager; no root needed).
pub fn forget_active_wifi() -> Result<String, String> {
    let out = sh("nmcli -t -f UUID,TYPE connection show --active 2>/dev/null");
    let mut uuid = None;
    for line in out.lines() {
        let mut fields = line.splitn(2, ':');
        let id = fields.next().unwrap_or("");
        let kind = fields.next().unwrap_or("");
        if kind.contains("wireless") || kind.contains("wifi") {
            uuid = Some(id.to_owned());
            break;
        }
    }
    let Some(uuid) = uuid else {
        return Err("No active Wi-Fi connection was found.".to_owned());
    };
    let result = Command::new("nmcli")
        .args(["connection", "delete", "uuid", &uuid])
        .output()
        .map_err(|e| format!("Could not run nmcli: {e}"))?;
    if result.status.success() {
        Ok("Wi-Fi network forgotten.".to_owned())
    } else {
        let err = String::from_utf8_lossy(&result.stderr).trim().to_owned();
        Err(if err.is_empty() {
            "Could not forget the network.".to_owned()
        } else {
            err
        })
    }
}

fn battery_state() -> (Option<u8>, bool) {
    let entries = match fs::read_dir("/sys/class/power_supply") {
        Ok(e) => e,
        Err(_) => return (None, false),
    };
    let mut pct = None;
    let mut charging = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("BAT") {
            continue;
        }
        let dir = entry.path();
        if let Ok(cap) = fs::read_to_string(dir.join("capacity")) {
            if let Ok(v) = cap.trim().parse::<u8>() {
                pct = Some(v);
            }
        }
        let status_path = dir.join("status");
        let status = read_trim(status_path.to_str().unwrap_or_default());
        if status == "Charging" || status == "Full" {
            charging = true;
        }
    }
    if pct.is_none() {
        let ac = fs::read_dir("/sys/class/power_supply")
            .ok()
            .map(|d| {
                d.flatten().any(|e| {
                    let online = e.path().join("online");
                    e.file_name().to_string_lossy().starts_with("AC")
                        && read_trim(online.to_str().unwrap_or_default()) == "1"
                })
            })
            .unwrap_or(false);
        charging = ac;
    }
    (pct, charging)
}

thread_local! {
    /// Every GSettings value, filled once per load by `dconf_begin`.
    static DCONF: std::cell::RefCell<Option<std::collections::HashMap<String, String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Read every schema default plus every user override in two processes, then
/// answer all later `gsettings_text` calls from memory. `Info::load` used to
/// spawn one `gsettings` process per key (40+), which dominated startup time.
fn dconf_begin() {
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if command_exists("gsettings") {
        for line in sh("gsettings list-recursively 2>/dev/null").lines() {
            let mut it = line.splitn(3, ' ');
            let (Some(schema), Some(key), Some(val)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            map.insert(
                format!("/{}/{}", schema.replace('.', "/"), key),
                val.trim().to_owned(),
            );
        }
    }
    if command_exists("dconf") {
        let dump = sh("dconf dump / 2>/dev/null");
        let mut section = String::new();
        for line in dump.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim_matches('/').to_owned();
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            let path = if section.is_empty() {
                format!("/{key}")
            } else {
                format!("/{section}/{key}")
            };
            map.insert(path, val.trim().to_owned());
        }
    }
    DCONF.with(|c| *c.borrow_mut() = Some(map));
    DESKTOP_ENTRIES.with(|c| *c.borrow_mut() = None);
    XDG_DEFAULTS.with(|c| *c.borrow_mut() = None);
}

fn gsettings_text(schema: &str, key: &str) -> String {
    let path = format!("/{}/{}", schema.replace('.', "/"), key);
    if let Some(v) = DCONF.with(|c| c.borrow().as_ref().and_then(|m| m.get(&path).cloned())) {
        return v;
    }
    sh(&format!("gsettings get {} {} 2>/dev/null", schema, key))
}

fn gs_bool(schema: &str, key: &str) -> Option<bool> {
    let raw = gsettings_text(schema, key);
    if raw.is_empty() {
        None
    } else {
        Some(raw.contains("true"))
    }
}

fn gs_uint(schema: &str, key: &str) -> Option<u32> {
    gsettings_text(schema, key)
        .split_whitespace()
        .last()
        .and_then(|v| v.parse::<u32>().ok())
}

fn bluetooth_devices() -> Vec<BluetoothDevice> {
    let out = sh("bluetoothctl devices 2>/dev/null");
    let mut devices = Vec::new();
    for line in out.lines() {
        let mut parts = line.splitn(3, ' ');
        if parts.next() != Some("Device") {
            continue;
        }
        let mac = parts.next().unwrap_or("").to_owned();
        let name = parts.next().unwrap_or("").to_owned();
        if mac.is_empty() {
            continue;
        }
        let info = sh(&format!("bluetoothctl info {} 2>/dev/null", mac));
        let paired = info.lines().any(|l| l.trim() == "Paired: yes");
        let connected = info.lines().any(|l| l.trim() == "Connected: yes");
        devices.push(BluetoothDevice {
            mac,
            name,
            paired,
            connected,
        });
        if devices.len() >= 10 {
            break;
        }
    }
    devices
}

fn wifi_networks(rescan: bool) -> Vec<WifiNetwork> {
    let out = sh(&format!(
        "nmcli -t --escape no -f SSID,SECURITY,SIGNAL,IN-USE device wifi list --rescan {} 2>/dev/null",
        if rescan { "yes" } else { "no" }
    ));
    let mut networks = Vec::new();
    for line in out.lines() {
        let fields = nmcli_split(line);
        if fields.len() < 4 || fields[0].trim().is_empty() {
            continue;
        }
        let ssid = fields[0].trim().to_owned();
        if networks
            .iter()
            .any(|network: &WifiNetwork| network.ssid == ssid)
        {
            continue;
        }
        networks.push(WifiNetwork {
            ssid,
            security: fields[1].trim().to_owned(),
            signal: fields[2].trim().parse().unwrap_or(0),
            active: fields[3].trim() == "*",
        });
        if networks.len() >= 16 {
            break;
        }
    }
    networks
}

/// Split an nmcli `-t` line on unescaped colons (\: is an escaped colon).
fn nmcli_split(line: &str) -> Vec<String> {
    const PLACEHOLDER: char = '\u{1}';
    line.replace("\\:", &PLACEHOLDER.to_string())
        .split(':')
        .map(|field| field.replace(PLACEHOLDER, ":"))
        .collect()
}

fn net_devices() -> Vec<NetDevice> {
    let out = sh("nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device status 2>/dev/null");
    let mut result = Vec::new();
    for line in out.lines() {
        let fields = nmcli_split(line);
        if fields.len() < 4 {
            continue;
        }
        let name = fields[0].to_owned();
        if name == "lo" {
            continue;
        }
        let address = sh(&format!(
            "nmcli -t -f IP4.ADDRESS device show {} 2>/dev/null | head -1 | cut -d: -f2 | cut -d/ -f1",
            shell_quote(&name)
        ));
        result.push(NetDevice {
            name,
            kind: fields[1].to_owned(),
            state: fields[2].to_owned(),
            connection: fields[3].to_owned(),
            address,
        });
        if result.len() >= 6 {
            break;
        }
    }
    result
}

/// Locate a themed PNG icon for a `.desktop` `Icon=` value.
pub fn find_icon(name: &str) -> Option<std::path::PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let direct = Path::new(name);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct.to_path_buf());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let mut roots = vec![
        "/usr/share/icons".to_owned(),
        "/var/lib/flatpak/exports/share/icons".to_owned(),
    ];
    if !home.is_empty() {
        roots.push(format!("{home}/.local/share/icons"));
        roots.push(format!("{home}/.local/share/flatpak/exports/share/icons"));
    }
    let themes = ["hicolor", "Yaru", "Adwaita", "breeze"];
    let sizes = ["256x256", "128x128", "96x96", "64x64", "48x48", "32x32"];
    for root in &roots {
        for theme in themes {
            for size in sizes {
                let candidate = Path::new(root)
                    .join(theme)
                    .join(size)
                    .join("apps")
                    .join(format!("{name}.png"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    for dir in ["/usr/share/pixmaps", "/var/lib/snapd/desktop/icons"] {
        let candidate = Path::new(dir).join(format!("{name}.png"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ---------------------------------------------------------------- actions

pub fn set_volume(pct: u32) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", pct)])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-volume",
            "@DEFAULT_AUDIO_SINK@",
            &format!("{:.2}", pct as f32 / 100.0),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_muted(muted: bool) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args([
                "set-sink-mute",
                "@DEFAULT_SINK@",
                if muted { "1" } else { "0" },
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-mute",
            "@DEFAULT_AUDIO_SINK@",
            if muted { "1" } else { "0" },
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_input_volume(pct: u32) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args([
                "set-source-volume",
                "@DEFAULT_SOURCE@",
                &format!("{}%", pct),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-volume",
            "@DEFAULT_AUDIO_SOURCE@",
            &format!("{:.2}", pct as f32 / 100.0),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_input_muted(muted: bool) -> bool {
    if command_exists("pactl") {
        return Command::new("pactl")
            .args([
                "set-source-mute",
                "@DEFAULT_SOURCE@",
                if muted { "1" } else { "0" },
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    Command::new("wpctl")
        .args([
            "set-mute",
            "@DEFAULT_AUDIO_SOURCE@",
            if muted { "1" } else { "0" },
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_default_source(name: &str) -> bool {
    if command_exists("pactl") {
        let ok = Command::new("pactl")
            .args(["set-default-source", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    let id = sources()
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .unwrap_or_else(|| name.to_owned());
    if id.is_empty() {
        return false;
    }
    Command::new("wpctl")
        .args(["set-default", &id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_default_sink(name: &str) -> bool {
    if command_exists("pactl") {
        let ok = Command::new("pactl")
            .args(["set-default-sink", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    // wpctl wants a numeric id: find the sink whose node.name matches.
    let id = sinks()
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .unwrap_or_else(|| name.to_owned());
    if id.is_empty() {
        return false;
    }
    Command::new("wpctl")
        .args(["set-default", &id])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn audio_available() -> bool {
    command_exists("pactl") || command_exists("wpctl")
}

pub fn test_sound() {
    let candidates = [
        "/usr/share/sounds/freedesktop/stereo/complete.oga",
        "/usr/share/sounds/gnome/default/alerts/bell.oga",
        "/usr/share/sounds/ubuntu/stereo/bell.ogg",
    ];
    let players: [(&str, bool); 3] = [
        ("paplay", false),
        ("pw-play", false),
        ("canberra-gtk-play", true),
    ];
    for path in candidates {
        if !Path::new(path).exists() {
            continue;
        }
        for (player, use_file_flag) in players {
            if !command_exists(player) {
                continue;
            }
            let mut cmd = Command::new(player);
            if use_file_flag {
                cmd.arg("-f");
            }
            if cmd.arg(path).spawn().is_ok() {
                return;
            }
        }
    }
}

pub fn set_notifications(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.notifications", "show-banners", on)
}

pub fn set_lock_notifications(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.notifications", "show-in-lock-screen", on)
}

pub fn set_night_light(on: bool) -> bool {
    gs_set_bool(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-enabled",
        on,
    )
}

fn gs_set_bool(schema: &str, key: &str, value: bool) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, if value { "true" } else { "false" }])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_power_profile(profile: &str) -> bool {
    Command::new("powerprofilesctl")
        .args(["set", profile])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_idle_delay(seconds: u32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.session",
            "idle-delay",
            &format!("uint32 {}", seconds),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_clock_24h(on: bool) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.interface",
            "clock-format",
            if on { "24h" } else { "12h" },
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_clock_detail(key: &str, on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.interface", key, on)
}

pub fn set_mouse_speed(value: f32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.peripherals.mouse",
            "speed",
            &format!("{:.3}", value.clamp(-1.0, 1.0)),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_mouse(flag: &str, on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.peripherals.mouse", flag, on)
}

pub fn set_touchpad(flag: &str, on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.peripherals.touchpad", flag, on)
}

pub fn set_touchpad_speed(value: f32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.peripherals.touchpad",
            "speed",
            &format!("{:.3}", value.clamp(-1.0, 1.0)),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_key_repeat(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.peripherals.keyboard", "repeat", on)
}

pub fn set_repeat_delay(ms: u32) -> bool {
    gsettings_set_uint("org.gnome.desktop.peripherals.keyboard", "delay", ms)
}

pub fn set_repeat_interval(ms: u32) -> bool {
    gsettings_set_uint(
        "org.gnome.desktop.peripherals.keyboard",
        "repeat-interval",
        ms,
    )
}

fn gsettings_set_uint(schema: &str, key: &str, value: u32) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, &format!("uint32 {value}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Suspend when idle on AC / battery. `true` = suspend, `false` = do nothing.
pub fn set_sleep_idle(ac: bool, battery: bool) -> bool {
    let ac_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "sleep-inactive-ac-type",
        if ac { "suspend" } else { "nothing" },
    );
    let battery_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "sleep-inactive-battery-type",
        if battery { "suspend" } else { "nothing" },
    );
    ac_ok && battery_ok
}

/// Lid close behaviour: pass "suspend" or "nothing" for each power source.
pub fn set_lid_action(ac: &str, battery: &str) -> bool {
    let ac_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "lid-close-ac-action",
        ac,
    );
    let battery_ok = gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "lid-close-battery-action",
        battery,
    );
    ac_ok && battery_ok
}

fn gsettings_set_enum(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Set the wallpaper for light, dark and the lock screen from a local file.
pub fn set_wallpaper(path: &str) -> bool {
    let uri = if path.starts_with("file://") {
        path.to_owned()
    } else {
        format!("file://{}", path.replace(' ', "%20"))
    };
    let mut ok = true;
    for (schema, key) in [
        ("org.gnome.desktop.background", "picture-uri"),
        ("org.gnome.desktop.background", "picture-uri-dark"),
        ("org.gnome.desktop.screensaver", "picture-uri"),
    ] {
        ok = gsettings_set_string(schema, key, &uri) && ok;
    }
    ok
}

fn gsettings_set_string(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, &format!("'{value}'")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Write a GVariant literal (arrays, tuples) without adding quotes.
fn gsettings_set_raw(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_bluetooth(on: bool) -> Result<(), String> {
    let out = Command::new("bluetoothctl")
        .args(["power", if on { "on" } else { "off" }])
        .output()
        .map_err(|e| format!("Could not run bluetoothctl: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

pub fn connect_bluetooth(mac: &str, connect: bool) -> Result<(), String> {
    let out = Command::new("bluetoothctl")
        .args([if connect { "connect" } else { "disconnect" }, mac])
        .output()
        .map_err(|e| format!("Could not run bluetoothctl: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let out = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        Err(if err.is_empty() { out } else { err })
    }
}

pub fn pair_bluetooth(mac: &str) -> Result<(), String> {
    let out = Command::new("bluetoothctl")
        .args(["--timeout", "25", "pair", mac])
        .output()
        .map_err(|e| format!("Could not run bluetoothctl: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let out = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        Err(if err.is_empty() { out } else { err })
    }
}

pub fn scan_bluetooth() -> Result<(), String> {
    // `--timeout` stops the scan itself; `timeout` is a hard cap in case
    // bluetoothctl or the adapter wedges and never returns.
    let mut command = if command_exists("timeout") {
        let mut command = Command::new("timeout");
        command.args(["20", "bluetoothctl", "--timeout", "8", "scan", "on"]);
        command
    } else {
        let mut command = Command::new("bluetoothctl");
        command.args(["--timeout", "8", "scan", "on"]);
        command
    };
    let out = command
        .output()
        .map_err(|e| format!("Could not run bluetoothctl: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

pub fn connect_wifi(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let mut command = Command::new("nmcli");
    command.args(["device", "wifi", "connect", ssid]);
    if let Some(password) = password.filter(|value| !value.is_empty()) {
        command.args(["password", password]);
    }
    let out = command
        .output()
        .map_err(|e| format!("Could not run nmcli: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let out = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        Err(if err.is_empty() { out } else { err })
    }
}

/// Change the panel backlight through systemd-logind, which grants the active
/// session write access without root (the same path GNOME Settings uses).
fn logind_brightness(device: &str, value: u32) -> bool {
    let call = if command_exists("busctl") {
        sh(&format!(
            "busctl call org.freedesktop.login1 /org/freedesktop/login1/session/auto \
             org.freedesktop.login1.Session SetBrightness ssu backlight {} {} 2>/dev/null",
            shell_quote(device),
            value
        ))
    } else {
        sh(&format!(
            "gdbus call --system --dest org.freedesktop.login1 \
             --object-path /org/freedesktop/login1/session/auto \
             --method org.freedesktop.login1.Session.SetBrightness backlight {} {} 2>/dev/null",
            shell_quote(device),
            value
        ))
    };
    let _ = call;
    read_trim(&format!("/sys/class/backlight/{device}/brightness")) == value.to_string()
}

pub fn set_brightness(pct: u8) -> Result<(), String> {
    // logind first: it needs no password prompt and works for the active session.
    if let Ok(entries) = fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let device = entry.file_name().to_string_lossy().into_owned();
            let Ok(max) = fs::read_to_string(entry.path().join("max_brightness")) else {
                continue;
            };
            let Ok(max) = max.trim().parse::<f32>() else {
                continue;
            };
            let value = ((max * pct as f32 / 100.0).round() as u32).max(1);
            if logind_brightness(&device, value) {
                return Ok(());
            }
        }
    }
    // Prefer brightnessctl (works for users in the video group, no password prompt).
    if command_exists("brightnessctl") {
        let out = Command::new("brightnessctl")
            .args(["set", &format!("{}%", pct)])
            .output()
            .map_err(|e| e.to_string())?;
        return if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        };
    }
    // Fall back to writing sysfs directly; this only works when the node is writable.
    if let Ok(entries) = fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let dir = entry.path();
            if let (Ok(max), Ok(_cur)) = (
                fs::read_to_string(dir.join("max_brightness")),
                fs::read_to_string(dir.join("brightness")),
            ) {
                if let Ok(max) = max.trim().parse::<f32>() {
                    let value = (max * pct as f32 / 100.0).round() as u32;
                    if fs::write(dir.join("brightness"), value.to_string()).is_ok() {
                        return Ok(());
                    }
                }
            }
        }
    }
    Err("Backlight is not writable. Install brightnessctl and join the video group.".to_owned())
}

pub fn rename_hostname(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 63
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return Err("Use only letters, digits, '-' and '.' (max 63 characters).".to_owned());
    }
    if !command_exists("pkexec") {
        return Err("pkexec (PolicyKit) is required to change the device name.".to_owned());
    }
    let out = Command::new("pkexec")
        .args(["hostnamectl", "set-hostname", name])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        if err.is_empty() {
            Err("Authentication was cancelled.".to_owned())
        } else {
            Err(err)
        }
    }
}

static PHASED_UPDATES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True when the pending upgrades are currently held back by Ubuntu's
/// phased-update rollout. `apt-get upgrade` then reports success while
/// changing nothing, which looked like "Install updates does nothing".
pub fn updates_phased() -> bool {
    PHASED_UPDATES.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn check_updates() -> Vec<PackageUpdate> {
    PHASED_UPDATES.store(
        sh("timeout 60 apt-get -s upgrade 2>/dev/null").contains("deferred due to phasing"),
        std::sync::atomic::Ordering::Relaxed,
    );
    let out = sh("timeout 60 apt list --upgradable 2>/dev/null");
    let mut packages = Vec::new();
    for line in out.lines() {
        if line.starts_with("Listing") || line.trim().is_empty() {
            continue;
        }
        let Some((name, rest)) = line.split_once('/') else {
            continue;
        };
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        // `name/repo version arch [upgradable from: old]`
        let new = fields.get(1).copied().unwrap_or("").to_owned();
        let old = line
            .split("[upgradable from: ")
            .nth(1)
            .and_then(|v| v.split(']').next())
            .unwrap_or("")
            .to_owned();
        let name = name.trim();
        if !name.is_empty() {
            packages.push(PackageUpdate {
                name: name.to_owned(),
                old,
                new,
                source: "apt".to_owned(),
            });
        }
    }
    packages.extend(snap_updates());
    packages.extend(flatpak_updates());
    packages
}

/// Snap apps with a newer revision, like App Hub lists them. Bases, snapd,
/// kernels and gadgets update themselves and are not shown.
fn snap_updates() -> Vec<PackageUpdate> {
    if !command_exists("snap") {
        return Vec::new();
    }
    let installed: std::collections::HashMap<String, String> = sh("snap list 2>/dev/null")
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f = l.split_whitespace().collect::<Vec<_>>();
            Some((f.first()?.to_string(), f.get(1)?.to_string()))
        })
        .collect();
    sh("timeout 60 snap refresh --list 2>/dev/null")
        .lines()
        .skip(1)
        .filter_map(|line| {
            let f = line.split_whitespace().collect::<Vec<_>>();
            if f.len() < 3 {
                return None;
            }
            let name = f[0];
            let notes = f.last().copied().unwrap_or("");
            let system = ["base", "snapd", "kernel", "gadget", "core"]
                .iter()
                .any(|kind| notes.contains(kind))
                || name == "snapd"
                || name.starts_with("core");
            if system {
                return None;
            }
            Some(PackageUpdate {
                name: name.to_owned(),
                old: installed.get(name).cloned().unwrap_or_default(),
                new: f[1].to_owned(),
                source: "snap".to_owned(),
            })
        })
        .collect()
}

/// Flatpak apps (system and user) with an update on their remote.
fn flatpak_updates() -> Vec<PackageUpdate> {
    if !command_exists("flatpak") {
        return Vec::new();
    }
    sh("timeout 90 flatpak remote-ls --updates --app --columns=application,version 2>/dev/null")
        .lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let id = cols.next()?.trim();
            if id.is_empty() || id.eq_ignore_ascii_case("application id") {
                return None;
            }
            Some(PackageUpdate {
                name: id.to_owned(),
                old: String::new(),
                new: cols.next().unwrap_or("").trim().to_owned(),
                source: "flatpak".to_owned(),
            })
        })
        .collect()
}

pub fn install_package(package: &str) -> std::sync::mpsc::Receiver<crate::jobs::JobEvent> {
    let mut cmd = Command::new("pkexec");
    cmd.arg("env")
        .arg("DEBIAN_FRONTEND=noninteractive")
        .args([
            "apt-get",
            "-y",
            "-o",
            "Dpkg::Options::=--force-confdef",
            "-o",
            "Dpkg::Options::=--force-confold",
            "install",
            package,
        ])
        .stdin(Stdio::null());
    crate::jobs::spawn(cmd)
}

/// `apt-get update` through PolicyKit; streams output lines.
pub fn refresh_package_lists() -> std::sync::mpsc::Receiver<crate::jobs::JobEvent> {
    let mut cmd = Command::new("pkexec");
    cmd.arg("env")
        .arg("DEBIAN_FRONTEND=noninteractive")
        .args([
            "apt-get",
            "-o",
            "APT::Status-Fd=1",
            "-o",
            "Acquire::http::Timeout=20",
            "update",
        ])
        .stdin(Stdio::null());
    crate::jobs::spawn(cmd)
}

/// Install every pending update: apt packages, snap apps and Flatpak apps.
/// One password prompt for the system part, streamed live; user-installed
/// Flatpaks update afterwards without a password.
pub fn install_updates() -> std::sync::mpsc::Receiver<crate::jobs::JobEvent> {
    // Ubuntu phases some upgrades; installing is an explicit user action, so
    // include them instead of silently doing nothing. The apt result decides
    // the exit status; snap and Flatpak report their own errors in the log.
    let root = "export DEBIAN_FRONTEND=noninteractive; \
        apt-get -y -o APT::Status-Fd=1 -o APT::Get::Always-Include-Phased-Updates=true \
        -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold upgrade; \
        status=$?; \
        if command -v snap >/dev/null; then echo 'Updating snap apps…'; snap refresh; fi; \
        if command -v flatpak >/dev/null; then echo 'Updating Flatpak apps (system)…'; \
        flatpak update --system -y --noninteractive; fi; \
        exit $status";
    let script = format!(
        "pkexec sh -c {}; status=$?; \
         if [ $status -eq 0 ] && command -v flatpak >/dev/null; then \
         echo 'Updating Flatpak apps (user)…'; flatpak update --user -y --noninteractive; fi; \
         exit $status",
        shell_quote(root)
    );
    let mut cmd = Command::new("sh");
    cmd.args(["-c", &script]).stdin(Stdio::null());
    crate::jobs::spawn(cmd)
}

pub fn reboot_required() -> bool {
    Path::new("/var/run/reboot-required").exists()
}

pub fn reboot_packages() -> Vec<String> {
    fs::read_to_string("/var/run/reboot-required.pkgs")
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| l.rsplit('/').next().unwrap_or(l).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Reboot the machine. An active desktop session can reboot through logind
/// without a prompt; otherwise fall back to pkexec with a path that exists on
/// every Ubuntu system. Returns a message for the toast/activity log.
pub fn schedule_reboot() -> Result<String, String> {
    if command_exists("systemctl") {
        match Command::new("systemctl").arg("reboot").output() {
            Ok(out) if out.status.success() => return Ok("The system is restarting.".to_owned()),
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                if code == 0 {
                    return Ok("The system is restarting.".to_owned());
                }
                // Polkit may refuse a non-interactive reboot; try pkexec below.
                let _ = out;
            }
            Err(_) => {}
        }
    }
    let mut last_err = "Could not reboot the system.".to_owned();
    for args in [
        &["systemctl", "reboot"][..],
        &["/usr/sbin/shutdown", "-r", "+1"][..],
        &["/sbin/shutdown", "-r", "+1"][..],
    ] {
        if !command_exists("pkexec") {
            break;
        }
        match Command::new("pkexec").args(args).output() {
            Ok(out) if out.status.success() => {
                return Ok(if args[0] == "systemctl" {
                    "The system is restarting.".to_owned()
                } else {
                    "Restart scheduled; the system will reboot in one minute.".to_owned()
                });
            }
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                if code == 126 || code == 127 {
                    return Err("Authentication was cancelled.".to_owned());
                }
                let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
                if !err.is_empty() {
                    last_err = err;
                }
            }
            Err(err) => last_err = format!("pkexec failed: {err}"),
        }
    }
    Err(last_err)
}

// ---------------------------------------------------------------- appearance

pub struct Appearance {
    pub mode: Mode,
    pub accent_name: String,
    pub accent: Option<(u8, u8, u8)>,
}

pub const GNOME_ACCENTS: [(&str, (u8, u8, u8)); 9] = [
    ("blue", (0x35, 0x84, 0xE4)),
    ("teal", (0x21, 0x90, 0xA4)),
    ("green", (0x3A, 0x94, 0x4A)),
    ("yellow", (0xC8, 0x88, 0x00)),
    ("orange", (0xED, 0x5B, 0x00)),
    ("red", (0xE6, 0x2D, 0x42)),
    ("pink", (0xD5, 0x61, 0x99)),
    ("purple", (0x91, 0x41, 0xAC)),
    ("slate", (0x6F, 0x83, 0x96)),
];

pub fn appearance() -> Appearance {
    let scheme = sh("gsettings get org.gnome.desktop.interface color-scheme")
        .trim_matches('\'')
        .to_owned();
    let theme = sh("gsettings get org.gnome.desktop.interface gtk-theme")
        .trim_matches('\'')
        .to_lowercase();
    let mode = if scheme == "prefer-dark" || (scheme.is_empty() && theme.contains("dark")) {
        Mode::Dark
    } else if scheme == "prefer-light" {
        Mode::Light
    } else if scheme.is_empty() {
        // No desktop bus at all: assume dark so the app still looks right.
        Mode::Dark
    } else {
        Mode::Light
    };

    let mut accent_name = sh("gsettings get org.gnome.desktop.interface accent-color")
        .trim_matches('\'')
        .to_owned();
    if accent_name.is_empty() {
        // Older desktops encode the accent in the theme name (Yaru-blue).
        accent_name = theme.rsplit('-').next().unwrap_or("").to_owned();
    }
    let accent = GNOME_ACCENTS
        .iter()
        .find(|(name, _)| *name == accent_name)
        .map(|(_, rgb)| *rgb);

    Appearance {
        mode,
        accent_name,
        accent,
    }
}

/// Where the chosen interface design is remembered between launches.
fn design_path() -> Option<std::path::PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("win11-settings").join("ui"))
}

/// `~/.config/harbor/<name>`: settings shared by every Harbor app.
fn harbor_path(name: &str) -> Option<PathBuf> {
    design_path().and_then(|p| p.parent()?.parent().map(|c| c.join("harbor").join(name)))
}

fn write_config(path: Option<PathBuf>, text: &str) {
    let Some(path) = path else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, text);
}

/// Where a glass style is set: the desktop (Fusion Desk, "global"), every
/// Harbor app, or one app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GlassScope {
    Global,
    Harbor,
    Settings,
    Files,
    TaskManager,
}

impl GlassScope {
    pub const ALL: [GlassScope; 5] = [
        GlassScope::Global,
        GlassScope::Harbor,
        GlassScope::Settings,
        GlassScope::Files,
        GlassScope::TaskManager,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GlassScope::Global => "Global",
            GlassScope::Harbor => "All Harbor apps",
            GlassScope::Settings => "Settings",
            GlassScope::Files => "Files",
            GlassScope::TaskManager => "Task Manager",
        }
    }

    /// The scope this one follows when it has no values of its own.
    pub fn parent(self) -> Option<GlassScope> {
        match self {
            GlassScope::Global => None,
            GlassScope::Harbor => Some(GlassScope::Global),
            _ => Some(GlassScope::Harbor),
        }
    }

    /// `~/.config/harbor/<file>`; Python apps read the same files.
    fn file(self) -> &'static str {
        match self {
            GlassScope::Global => "glass-global",
            GlassScope::Harbor => "glass",
            GlassScope::Settings => "glass-settings",
            GlassScope::Files => "glass-files",
            GlassScope::TaskManager => "glass-taskmgr",
        }
    }
}

/// A scope's own values; `None` follows its parent scope. All Harbor apps
/// start with glass off; each app starts following them.
pub fn scope_glass(scope: GlassScope) -> Option<crate::theme::Glass> {
    use crate::theme::Glass;
    if scope == GlassScope::Global {
        return Some(global_glass());
    }
    let mut text = harbor_path(scope.file()).and_then(|p| fs::read_to_string(p).ok());
    if text.is_none() && scope == GlassScope::Settings {
        // 0.5.18 previews kept this app's choice next to its design file.
        text = design_path().and_then(|p| fs::read_to_string(p.with_file_name("glass")).ok());
    }
    match text.as_deref().map(str::trim) {
        None if scope == GlassScope::Harbor => Some(Glass::OFF),
        None | Some("") | Some("global") | Some("harbor") => None,
        Some(t) => Some(Glass::from_text(t)),
    }
}

/// Save a scope's values, or make it follow its parent (`None`). The global
/// scope is Fusion Desk's own glass settings when it is installed.
pub fn save_scope_glass(scope: GlassScope, glass: Option<crate::theme::Glass>) -> bool {
    if scope == GlassScope::Global {
        let glass = glass.unwrap_or_default();
        write_config(harbor_path(scope.file()), &glass.to_text());
        return !crate::desktop::fusion_installed() || crate::desktop::set_fusion_glass(&glass);
    }
    let follow = if scope == GlassScope::Harbor {
        "global"
    } else {
        "harbor"
    };
    let text = glass
        .map(|g| g.to_text())
        .unwrap_or_else(|| follow.to_owned());
    write_config(harbor_path(scope.file()), &text);
    true
}

/// The desktop's glass: Fusion Desk's settings, or Harbor's copy of them
/// when Fusion Desk is not installed.
pub fn global_glass() -> crate::theme::Glass {
    crate::desktop::fusion_glass().unwrap_or_else(|| {
        harbor_path(GlassScope::Global.file())
            .and_then(|p| fs::read_to_string(p).ok())
            .map(|t| crate::theme::Glass::from_text(&t))
            .unwrap_or_default()
    })
}

/// The values a scope paints with, after following its parents.
pub fn resolve_glass(scope: GlassScope) -> crate::theme::Glass {
    let mut at = scope;
    loop {
        if let Some(g) = scope_glass(at) {
            return g;
        }
        match at.parent() {
            Some(p) => at = p,
            None => return crate::theme::Glass::OFF,
        }
    }
}

/// The glass this app paints with. `WIN11_SETTINGS_GLASS` (`off`, `glossy`,
/// `liquid` or `key=value,...`) overrides it for screenshot runs.
pub fn effective_glass() -> crate::theme::Glass {
    if let Ok(v) = std::env::var("WIN11_SETTINGS_GLASS") {
        return crate::theme::Glass::from_text(&v);
    }
    resolve_glass(GlassScope::Settings)
}

/// Whether the Windows 10 home cards list their pages. On unless turned off;
/// `WIN11_SETTINGS_HOME_LINKS=0|1` overrides it for screenshot runs.
pub fn saved_home_links() -> bool {
    if let Ok(value) = std::env::var("WIN11_SETTINGS_HOME_LINKS") {
        return value != "0";
    }
    design_path()
        .and_then(|p| fs::read_to_string(p.with_file_name("home-links")).ok())
        .map(|text| text.trim() != "off")
        .unwrap_or(true)
}

pub fn save_home_links(on: bool) {
    if std::env::var("WIN11_SETTINGS_HOME_LINKS").is_ok() {
        return;
    }
    let Some(path) = design_path().map(|p| p.with_file_name("home-links")) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, if on { "on" } else { "off" });
}

/// The interface design chosen in Personalisation. `WIN11_SETTINGS_UI`
/// overrides the saved choice (used by the screenshot and test runs).
pub fn saved_design() -> Option<crate::theme::Design> {
    if let Ok(value) = std::env::var("WIN11_SETTINGS_UI") {
        if let Some(design) = crate::theme::Design::from_key(&value) {
            return Some(design);
        }
    }
    let text = fs::read_to_string(design_path()?).ok()?;
    crate::theme::Design::from_key(&text)
}

/// Remember the interface design. The environment override is a test hook, so
/// it is deliberately not written back.
pub fn save_design(design: crate::theme::Design) {
    if std::env::var("WIN11_SETTINGS_UI").is_ok() {
        return;
    }
    let Some(path) = design_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, design.key());
}

pub fn set_color_scheme(mode: Mode) -> bool {
    let value = match mode {
        Mode::Dark => "prefer-dark",
        Mode::Light => "prefer-light",
    };
    Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", "color-scheme", value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Apply the closest GNOME accent to the desktop (Ubuntu 24.10+ key).
pub fn set_accent(r: u8, g: u8, b: u8) -> bool {
    if !command_exists("gsettings") {
        return false;
    }
    let name = GNOME_ACCENTS
        .iter()
        .min_by_key(|(_, rgb)| {
            let dr = rgb.0 as i32 - r as i32;
            let dg = rgb.1 as i32 - g as i32;
            let db = rgb.2 as i32 - b as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|(name, _)| *name)
        .unwrap_or("blue");
    Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", "accent-color", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------- accessibility

#[derive(Clone, Default)]
pub struct A11y {
    pub text_scale: f32,
    pub large_text: bool,
    pub visual_alerts: bool,
    pub sticky_keys: bool,
    pub mouse_keys: bool,
    pub animations: bool,
}

pub fn accessibility() -> A11y {
    A11y {
        text_scale: gsettings_text("org.gnome.desktop.interface", "text-scaling-factor")
            .parse::<f32>()
            .unwrap_or(1.0),
        large_text: gsettings_text("org.gnome.desktop.interface", "text-scaling-factor")
            .parse::<f32>()
            .map(|v| v >= 1.15)
            .unwrap_or(false),
        visual_alerts: gs_bool("org.gnome.desktop.wm.preferences", "visual-bell").unwrap_or(false),
        sticky_keys: gs_bool("org.gnome.desktop.a11y.keyboard", "stickykeys-enable")
            .unwrap_or(false),
        mouse_keys: gs_bool("org.gnome.desktop.a11y.keyboard", "mousekeys-enable").unwrap_or(false),
        animations: gs_bool("org.gnome.desktop.interface", "enable-animations").unwrap_or(true),
    }
}

pub fn set_text_scale(scale: f32) -> bool {
    Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.interface",
            "text-scaling-factor",
            &format!("{:.2}", scale),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn set_visual_alerts(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.wm.preferences", "visual-bell", on)
}

pub fn set_sticky_keys(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.a11y.keyboard", "stickykeys-enable", on)
}

pub fn set_mouse_keys(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.a11y.keyboard", "mousekeys-enable", on)
}

pub fn set_animations(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.interface", "enable-animations", on)
}

// ---------------------------------------------------------------- privacy

#[derive(Clone, Default)]
pub struct Privacy {
    pub screen_lock: bool,
    pub location: bool,
    pub camera: bool,
    pub microphone: bool,
    pub remember_recent: bool,
    pub remove_temp: bool,
    pub report_problems: bool,
}

pub fn privacy() -> Privacy {
    Privacy {
        screen_lock: gs_bool("org.gnome.desktop.screensaver", "lock-enabled").unwrap_or(true),
        location: gs_bool("org.gnome.system.location", "enabled").unwrap_or(false),
        camera: !gs_bool("org.gnome.desktop.privacy", "disable-camera").unwrap_or(false),
        microphone: !gs_bool("org.gnome.desktop.privacy", "disable-microphone").unwrap_or(false),
        remember_recent: gs_bool("org.gnome.desktop.privacy", "remember-recent-files")
            .unwrap_or(true),
        remove_temp: gs_bool("org.gnome.desktop.privacy", "remove-old-temp-files").unwrap_or(false),
        report_problems: gs_bool("org.gnome.desktop.privacy", "report-technical-problems")
            .unwrap_or(false),
    }
}

pub fn set_screen_lock(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.screensaver", "lock-enabled", on)
}

pub fn set_location(on: bool) -> bool {
    gs_set_bool("org.gnome.system.location", "enabled", on)
}

pub fn set_camera(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.privacy", "disable-camera", !on)
}

pub fn set_microphone(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.privacy", "disable-microphone", !on)
}

pub fn set_remember_recent(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.privacy", "remember-recent-files", on)
}

pub fn set_remove_temp(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.privacy", "remove-old-temp-files", on)
}

pub fn set_report_problems(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.privacy", "report-technical-problems", on)
}

/// Open a local path in the file manager.
/// Read the GDM autologin switch, or `None` when GDM is not installed.
fn read_autologin() -> Option<bool> {
    let text = fs::read_to_string("/etc/gdm3/custom.conf").ok()?;
    let mut in_daemon = false;
    let mut enabled = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_daemon = line == "[daemon]";
            continue;
        }
        if !in_daemon {
            continue;
        }
        if let Some(v) = line.strip_prefix("AutomaticLoginEnable=") {
            enabled = v.trim().eq_ignore_ascii_case("true");
        }
    }
    Some(enabled)
}

/// Turn automatic (password-free) login on or off in `/etc/gdm3/custom.conf`.
pub fn set_autologin(on: bool, user: &str) -> Result<(), String> {
    if !command_exists("pkexec") {
        return Err("pkexec is required to change the login policy".to_owned());
    }
    let value = if on { "true" } else { "false" };
    let user = user.replace(['\'', '\n', ' '], "");
    let mut script = String::from(
        "f=/etc/gdm3/custom.conf; [ -f \"$f\" ] || exit 1; \
         if grep -q '^AutomaticLoginEnable=' \"$f\"; then sed -i 's/^AutomaticLoginEnable=.*/AutomaticLoginEnable=VAL/' \"$f\"; \
         else sed -i '/^\\[daemon\\]/a AutomaticLoginEnable=VAL' \"$f\"; fi",
    )
    .replace("VAL", value);
    if on {
        script.push_str(&format!(
            "; if grep -q '^AutomaticLogin=' \"$f\"; then sed -i 's/^AutomaticLogin=.*/AutomaticLogin={user}/' \"$f\"; \
             else sed -i '/^\\[daemon\\]/a AutomaticLogin={user}' \"$f\"; fi"
        ));
    }
    status_ok(
        Command::new("pkexec")
            .args(["bash", "-c", &script])
            .status(),
        "The login manager refused the change",
    )
}

/// This account's AccountsService object path on the system bus.
fn account_object_path() -> Result<String, String> {
    let uid = sh("id -u");
    let reply = sh(&format!(
        "gdbus call --system --dest org.freedesktop.Accounts --object-path /org/freedesktop/Accounts --method org.freedesktop.Accounts.FindUserById {} 2>&1",
        uid.trim()
    ));
    let start = reply
        .find('\'')
        .ok_or_else(|| clean_error(&reply, "Could not find this account"))?;
    let len = reply[start + 1..]
        .find('\'')
        .ok_or_else(|| clean_error(&reply, "Could not find this account"))?;
    Ok(reply[start + 1..start + 1 + len].to_owned())
}

/// The account picture: AccountsService's IconFile (what the login screen
/// shows), else `~/.face`. `WIN11_SETTINGS_PHOTO` overrides it for tests.
pub fn user_photo() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("WIN11_SETTINGS_PHOTO") {
        return Some(PathBuf::from(path)).filter(|p| p.is_file());
    }
    if let Ok(object) = account_object_path() {
        let reply = sh(&format!(
            "gdbus call --system --dest org.freedesktop.Accounts --object-path {} --method org.freedesktop.DBus.Properties.Get org.freedesktop.Accounts.User IconFile 2>/dev/null",
            shell_quote(&object)
        ));
        if let Some(start) = reply.find('\'') {
            if let Some(len) = reply[start + 1..].find('\'') {
                let path = PathBuf::from(&reply[start + 1..start + 1 + len]);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    let face = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".face");
    face.is_file().then_some(face)
}

/// Set (or with an empty path, remove) the account picture. AccountsService
/// keeps its own copy, so the login and lock screens show it too.
pub fn set_user_photo(path: &str) -> Result<(), String> {
    let object = account_object_path()?;
    let out = sh(&format!(
        "gdbus call --system --dest org.freedesktop.Accounts --object-path {} --method org.freedesktop.Accounts.User.SetIconFile {} 2>&1",
        shell_quote(&object),
        shell_quote(path)
    ));
    if out.contains("Error") || out.contains("error") {
        Err(clean_error(&out, "The account service refused the picture"))
    } else {
        Ok(())
    }
}

/// Read a picture and cut a centred square of `size` pixels for the avatar.
pub fn load_photo(path: &Path, size: u32) -> Option<(usize, Vec<u8>)> {
    let img = image::open(path).ok()?;
    let side = img.width().min(img.height());
    let x = (img.width() - side) / 2;
    let y = (img.height() - side) / 2;
    let square = img
        .crop_imm(x, y, side, side)
        .resize_exact(size, size, image::imageops::FilterType::Triangle)
        .to_rgba8();
    Some((size as usize, square.into_raw()))
}

/// Change the account's display name through AccountsService.
pub fn set_real_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("The name cannot be empty".to_owned());
    }
    let uid = sh("id -u");
    let reply = sh(&format!(
        "gdbus call --session --dest org.freedesktop.Accounts --object-path /org/freedesktop/Accounts --method org.freedesktop.Accounts.FindUserById {} 2>&1",
        uid.trim()
    ));
    let Some(start) = reply.find('\'') else {
        return Err(clean_error(&reply, "Could not find this account"));
    };
    let Some(len) = reply[start + 1..].find('\'') else {
        return Err(clean_error(&reply, "Could not find this account"));
    };
    let path = &reply[start + 1..start + 1 + len];
    let out = sh(&format!(
        "gdbus call --session --dest org.freedesktop.Accounts --object-path {} --method org.freedesktop.Accounts.User.SetRealName {} 2>&1",
        shell_quote(path),
        shell_quote(name)
    ));
    if out.contains("Error") || out.contains("error") {
        Err(clean_error(&out, "The account service refused the change"))
    } else {
        Ok(())
    }
}

/// Set the login password (authenticated by a polkit prompt).
pub fn set_password(password: &str) -> Result<(), String> {
    use std::io::Write;
    if password.len() < 4 {
        return Err("Use at least 4 characters".to_owned());
    }
    if !command_exists("pkexec") {
        return Err("pkexec is required to change the password".to_owned());
    }
    let mut child = Command::new("pkexec")
        .arg("chpasswd")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start password change: {e}"))?;
    let user = std::env::var("USER").unwrap_or_else(|_| sh("id -un"));
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(format!("{user}:{password}\n").as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("Password change failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        Err(if err.is_empty() {
            "The password was not changed".to_owned()
        } else {
            err
        })
    }
}

fn clean_error(raw: &str, fallback: &str) -> String {
    let line = raw
        .lines()
        .find(|l| l.contains("Error") || l.contains("error"))
        .unwrap_or("");
    if line.is_empty() {
        fallback.to_owned()
    } else {
        line.split(": ").last().unwrap_or(line).trim().to_owned()
    }
}

fn status_ok(
    status: std::io::Result<std::process::ExitStatus>,
    fallback: &str,
) -> Result<(), String> {
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(fallback.to_owned()),
        Err(e) => Err(format!("{fallback}: {e}")),
    }
}

pub const NIGHT_LIGHT_DEFAULT_FROM: f32 = 20.0;
pub const NIGHT_LIGHT_DEFAULT_TO: f32 = 6.0;

/// A 00:00-24:00 window is how "no schedule" is represented in GSettings.
pub fn night_light_all_day(from: f32, to: f32) -> bool {
    from <= 0.01 && to >= 23.99
}

/// Turn the time window on or off. Off means the master switch applies at
/// once, which is what Windows does; GNOME otherwise keeps a manual window.
pub fn set_night_light_scheduled(on: bool, auto: bool, from: f32, to: f32) -> bool {
    if !on {
        return set_night_light_schedule(false, 0.0, 24.0);
    }
    if night_light_all_day(from, to) {
        return set_night_light_schedule(auto, NIGHT_LIGHT_DEFAULT_FROM, NIGHT_LIGHT_DEFAULT_TO);
    }
    set_night_light_schedule(auto, from, to)
}

pub fn set_night_light_schedule(auto: bool, from: f32, to: f32) -> bool {
    gs_set_bool(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-schedule-automatic",
        auto,
    ) && set_double(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-schedule-from",
        from,
    ) && set_double(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-schedule-to",
        to,
    )
}

pub fn set_night_light_temp(temp: u32) -> bool {
    gsettings_set_uint(
        "org.gnome.settings-daemon.plugins.color",
        "night-light-temperature",
        temp,
    )
}

/// Write a proxy configuration (mode: none/manual/auto).
pub fn set_proxy(
    mode: &str,
    http_host: &str,
    http_port: u32,
    socks_host: &str,
    socks_port: u32,
    ignore: &str,
) -> bool {
    let hosts = ignore
        .split([',', ' ', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("'{}'", s.replace('\'', "")))
        .collect::<Vec<_>>()
        .join(", ");
    gsettings_set_enum("org.gnome.system.proxy", "mode", mode)
        && gsettings_set_string("org.gnome.system.proxy.http", "host", http_host)
        && gsettings_set_uint("org.gnome.system.proxy.http", "port", http_port)
        && gsettings_set_string("org.gnome.system.proxy.socks", "host", socks_host)
        && gsettings_set_uint("org.gnome.system.proxy.socks", "port", socks_port)
        && gsettings_set_raw(
            "org.gnome.system.proxy",
            "ignore-hosts",
            &format!("[{hosts}]"),
        )
}

/// Turn the Wi-Fi radio itself on or off.
pub fn set_wifi_enabled(on: bool) -> Result<(), String> {
    let state = if on { "on" } else { "off" };
    let out = sh(&format!("nmcli radio wifi {state} 2>&1"));
    if out.contains("Error") || out.contains("not authorized") {
        Err(clean_error(&out, "NetworkManager refused the change"))
    } else {
        Ok(())
    }
}

/// Toggle network time synchronisation.
pub fn set_ntp(on: bool) -> Result<(), String> {
    if !command_exists("pkexec") {
        return Err("pkexec is required to change the clock source".to_owned());
    }
    let value = if on { "true" } else { "false" };
    let out = sh(&format!("pkexec timedatectl set-ntp {value} 2>&1"));
    if out.contains("Error") || out.contains("error") {
        Err(clean_error(&out, "The clock source was not changed"))
    } else {
        Ok(())
    }
}

/// Every locale the system can generate, cached after the first read.
pub fn list_locales() -> Vec<String> {
    static CACHE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut locales: Vec<String> = sh("localectl list-locales 2>/dev/null")
                .lines()
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty() && l.contains('_'))
                .collect();
            locales.sort();
            locales.dedup();
            locales
        })
        .clone()
}

/// Set the system language for future sessions.
pub fn set_locale(locale: &str) -> Result<(), String> {
    if !command_exists("pkexec") {
        return Err("pkexec is required to change the language".to_owned());
    }
    let locale = locale.replace(['\'', ' ', '\n'], "");
    let script =
        format!("locale-gen '{locale}' >/dev/null 2>&1; localectl set-locale LANG='{locale}'");
    let out = sh(&format!("pkexec bash -c {} 2>&1", shell_quote(&script)));
    if out.contains("Error") || out.contains("error") {
        Err(clean_error(&out, "The language was not changed"))
    } else {
        Ok(())
    }
}

pub fn set_cursor_size(px: u32) -> bool {
    gsettings_set_uint("org.gnome.desktop.interface", "cursor-size", px)
}

/// Power button behaviour: nothing | suspend | hibernate | interactive.
pub fn set_power_button(action: &str) -> bool {
    gsettings_set_enum(
        "org.gnome.settings-daemon.plugins.power",
        "power-button-action",
        action,
    )
}

pub fn set_low_battery_saver(on: bool) -> bool {
    gs_set_bool(
        "org.gnome.settings-daemon.plugins.power",
        "power-saver-profile-on-low-battery",
        on,
    )
}

pub fn set_discoverable(on: bool) -> Result<(), String> {
    let value = if on { "on" } else { "off" };
    let out = sh(&format!("bluetoothctl discoverable {value} 2>&1"));
    if out.to_lowercase().contains("failed") {
        Err(clean_error(&out, "Bluetooth refused the change"))
    } else {
        Ok(())
    }
}

fn set_double(schema: &str, key: &str, value: f32) -> bool {
    Command::new("gsettings")
        .args(["set", schema, key, &format!("{value}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn open_path(path: &str) -> bool {
    command_exists("gio")
        && Command::new("gio")
            .args(["open", path])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
}

/// Launch a `.desktop` file by path (the App Hub inventory exposes these).
pub fn launch_desktop(path: &str) -> bool {
    if !command_exists("gio") {
        return false;
    }
    Command::new("gio")
        .arg("launch")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}
