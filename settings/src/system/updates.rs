//! apt, snap and flatpak updates, unattended upgrades and reboots.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use super::*;

/// Whether unattended (automatic) security upgrades are enabled.
/// Reads the apt config; `None` when unattended-upgrades is not installed.
pub(crate) fn unattended_upgrades_enabled() -> Option<bool> {
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

pub(crate) static PHASED_UPDATES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

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
pub(crate) fn snap_updates() -> Vec<PackageUpdate> {
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
pub(crate) fn flatpak_updates() -> Vec<PackageUpdate> {
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
