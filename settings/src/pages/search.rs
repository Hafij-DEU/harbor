//! Settings search: the searchable index and the results page.

use eframe::egui::{self, Ui};

use crate::app::{App, Detail, Page, View};
use crate::icons::Icon;
use crate::theme::regular;
use crate::widgets::{self, Right};

use super::*;

pub struct SearchEntry {
    pub page: Page,
    pub detail: Option<Detail>,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub icon: Icon,
}

pub fn search_entries() -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for page in Page::ALL {
        entries.push(SearchEntry {
            page,
            detail: None,
            title: page.label(),
            subtitle: "Settings page",
            icon: page.icon(),
        });
    }
    let details = [
        (
            Detail::Display,
            "Resolution, brightness, night light",
            Icon::Display,
        ),
        (Detail::Sound, "Volume, output devices, mute", Icon::Sound),
        (
            Detail::Notifications,
            "Banners, lock screen alerts",
            Icon::Bell,
        ),
        (
            Detail::Power,
            "Power mode, battery, screen timeout",
            Icon::Battery,
        ),
        (Detail::Storage, "Disk space and volumes", Icon::Storage),
        (
            Detail::Advanced,
            "Tweaks: privacy, speed, services, startup",
            Icon::Sparkle,
        ),
        (
            Detail::About,
            "Device specifications and rename",
            Icon::Info,
        ),
    ];
    for (detail, subtitle, icon) in details {
        entries.push(SearchEntry {
            page: Page::System,
            detail: Some(detail),
            title: detail.title(),
            subtitle,
            icon,
        });
    }
    for (title, subtitle) in [
        ("Advanced network settings", "DNS servers, find fastest DNS, clear DNS cache"),
        ("DNS", "Cloudflare, Google, Quad9, AdGuard or custom DNS"),
    ] {
        entries.push(SearchEntry {
            page: Page::Network,
            detail: Some(Detail::AdvancedNetwork),
            title,
            subtitle,
            icon: Icon::Globe,
        });
    }
    entries.push(SearchEntry {
        page: Page::Personalisation,
        detail: None,
        title: "Accent colour",
        subtitle: "Change the highlight colour",
        icon: Icon::Personalise,
    });
    entries.push(SearchEntry {
        page: Page::Personalisation,
        detail: None,
        title: "Dark mode",
        subtitle: "Light or dark appearance",
        icon: Icon::Moon,
    });
    entries.push(SearchEntry {
        page: Page::TimeLanguage,
        detail: None,
        title: "Date & time",
        subtitle: "Time zone, clock format",
        icon: Icon::Clock,
    });
    for (page, title, subtitle, icon) in [
        (
            Page::Desktop,
            "Multitasking",
            "Hot corner, screen edges, workspaces, Alt+Tab",
            Icon::Apps,
        ),
        (
            Page::Desktop,
            "Ubuntu Dock",
            "Auto-hide, panel mode, icon size, position",
            Icon::Apps,
        ),
        (
            Page::Desktop,
            "Desktop icons",
            "Home folder, Trash, icon size",
            Icon::Image,
        ),
        (
            Page::Desktop,
            "Title bar buttons",
            "Minimize, maximize, window focus, title bar clicks",
            Icon::Apps,
        ),
        (
            Page::Desktop,
            "Fonts",
            "Interface, document and monospace fonts, hinting",
            Icon::Keyboard,
        ),
        (
            Page::Desktop,
            "Themes",
            "Legacy app theme, icons, cursor",
            Icon::Personalise,
        ),
        (
            Page::Desktop,
            "Startup apps",
            "Apps that start when you sign in",
            Icon::Power,
        ),
        (
            Page::Desktop,
            "Tweaks",
            "Overview key, middle-click paste, mouse acceleration, over-amplification",
            Icon::Sparkle,
        ),
        (
            Page::Extensions,
            "GNOME extensions",
            "Turn on or off, settings, install, remove",
            Icon::Hub,
        ),
    ] {
        entries.push(SearchEntry {
            page,
            detail: None,
            title,
            subtitle,
            icon,
        });
    }
    entries.push(SearchEntry {
        page: Page::Updates,
        detail: None,
        title: "Windows Update",
        subtitle: "Check for and install package updates",
        icon: Icon::Update,
    });
    entries
}

pub fn search(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let query = app.search.trim().to_lowercase();
    widgets::h1(ui, &pal, "Search results");
    ui.label(
        egui::RichText::new(format!("Results for “{}”", app.search.trim()))
            .font(regular(12.5))
            .color(pal.text_faint),
    );
    spacer(ui, 14.0);

    let entries = search_entries();
    let matches = entries
        .into_iter()
        .filter(|e| {
            e.title.to_lowercase().contains(&query)
                || e.subtitle.to_lowercase().contains(&query)
                || e.page.label().to_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        widgets::card(ui, &pal, |ui| {
            widgets::empty_note(
                ui,
                &pal,
                "No settings match that search. Try “display”, “sound” or “storage”.",
            );
        });
        return;
    }

    let mut target: Option<View> = None;
    widgets::card(ui, &pal, |ui| {
        for (index, entry) in matches.iter().enumerate() {
            if index > 0 {
                widgets::divider(ui, &pal);
            }
            let subtitle = format!("{} · {}", entry.page.label(), entry.subtitle);
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                entry.icon,
                entry.title,
                &subtitle,
                Right::Chevron,
            )
            .clicked()
            {
                target = Some(match entry.detail {
                    Some(detail) => View::Detail(entry.page, detail),
                    None => View::Page(entry.page),
                });
            }
        }
    });
    if let Some(view) = target {
        app.go(view);
    }
}
