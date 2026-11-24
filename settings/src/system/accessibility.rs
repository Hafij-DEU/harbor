//! Accessibility gsettings.

use std::process::Command;

use super::*;

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
