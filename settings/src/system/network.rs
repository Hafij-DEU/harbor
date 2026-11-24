//! Wi-Fi and wired (nmcli), Bluetooth (bluetoothctl), proxy and host name.

use std::process::Command;

use super::*;

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

pub(crate) fn bluetooth_devices() -> Vec<BluetoothDevice> {
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

pub(crate) fn wifi_networks(rescan: bool) -> Vec<WifiNetwork> {
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
pub(crate) fn nmcli_split(line: &str) -> Vec<String> {
    pub(crate) const PLACEHOLDER: char = '\u{1}';
    line.replace("\\:", &PLACEHOLDER.to_string())
        .split(':')
        .map(|field| field.replace(PLACEHOLDER, ":"))
        .collect()
}

pub(crate) fn net_devices() -> Vec<NetDevice> {
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

pub fn set_discoverable(on: bool) -> Result<(), String> {
    let value = if on { "on" } else { "off" };
    let out = sh(&format!("bluetoothctl discoverable {value} 2>&1"));
    if out.to_lowercase().contains("failed") {
        Err(clean_error(&out, "Bluetooth refused the change"))
    } else {
        Ok(())
    }
}
