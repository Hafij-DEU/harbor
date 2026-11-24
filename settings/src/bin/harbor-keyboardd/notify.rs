//! Desktop notifications from a root service: run `notify-send` as each
//! logged-in graphical user on their own session bus.

use std::process::{Command, Stdio};

/// Log the message and show it to every desktop user, without blocking keys.
pub fn notify(body: &str) {
    eprintln!("harbor-keyboardd: {body}");
    let body = body.to_owned();
    std::thread::spawn(move || send(&body));
}

fn send(body: &str) {
    let Ok(dir) = std::fs::read_dir("/run/user") else {
        return;
    };
    for entry in dir.flatten() {
        let Some(uid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let bus = entry.path().join("bus");
        if uid < 1000 || !bus.exists() {
            continue;
        }
        let _ = Command::new("setpriv")
            .args([
                &format!("--reuid={uid}"),
                &format!("--regid={}", gid_of(uid)),
                "--clear-groups",
                "env",
                &format!("DBUS_SESSION_BUS_ADDRESS=unix:path={}", bus.display()),
                "notify-send",
                "-a",
                "Harbor Settings",
                "-i",
                "input-keyboard",
                "Keyboard",
                body,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut c| c.wait());
    }
}

/// The primary group of a user, from /etc/passwd (falls back to the uid).
fn gid_of(uid: u32) -> u32 {
    std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|t| {
            t.lines().find_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                (f.len() > 3 && f[2] == uid.to_string()).then(|| f[3].parse().ok())?
            })
        })
        .unwrap_or(uid)
}
