//! User name, photo, password (AccountsService) and automatic login (GDM).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::*;

/// Open a local path in the file manager.
/// Read the GDM autologin switch, or `None` when GDM is not installed.
pub(crate) fn read_autologin() -> Option<bool> {
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
pub(crate) fn account_object_path() -> Result<String, String> {
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
    // AccountsService stores the picture without an extension: detect the
    // format from the contents.
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
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
