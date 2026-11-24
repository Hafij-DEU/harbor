//! The `Info` snapshot every page reads, its on-disk cache and the hardware probes.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::*;

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

pub(crate) fn cache_path() -> Option<std::path::PathBuf> {
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

pub(crate) fn save_cache(info: &Info) {
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

pub(crate) fn getent_full_name(user: &str) -> String {
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

pub(crate) fn os_field(os: &str, key: &str) -> String {
    os.lines()
        .find_map(|l| l.strip_prefix(&format!("{}=", key)))
        .map(|v| v.trim_matches('"').to_owned())
        .unwrap_or_default()
}

pub(crate) fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn human_desktop(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "gnome" => "GNOME".to_owned(),
        "ubuntu:gnome" => "Ubuntu (GNOME)".to_owned(),
        "kde" => "KDE Plasma".to_owned(),
        "xfce" => "Xfce".to_owned(),
        "unity" => "Unity".to_owned(),
        _ => raw.replace(':', " · "),
    }
}

pub(crate) fn cpu_model() -> String {
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

pub(crate) fn memory() -> Option<(f32, f32)> {
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

pub(crate) fn root_disk() -> (f32, f32, f32) {
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

pub(crate) fn mounts() -> Vec<Mount> {
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
