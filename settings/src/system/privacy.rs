//! Privacy, screen lock and notification gsettings.

use super::*;

pub fn set_notifications(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.notifications", "show-banners", on)
}

pub fn set_lock_notifications(on: bool) -> bool {
    gs_set_bool("org.gnome.desktop.notifications", "show-in-lock-screen", on)
}

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
