//! Navigation model: pages, detail sub-pages, Windows 10 categories and the current view.

use crate::icons::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    System,
    Bluetooth,
    Network,
    Personalisation,
    Desktop,
    Extensions,
    Apps,
    Accounts,
    Accessibility,
    Privacy,
    TimeLanguage,
    Compatibility,
    Updates,
    /// Advanced settings: system tweaks (own entry in the left menu).
    Advanced,
}

impl Page {
    // New pages go last: the index is saved in the Recently accessed file.
    pub const ALL: [Page; 14] = [
        Page::System,
        Page::Bluetooth,
        Page::Network,
        Page::Personalisation,
        Page::Desktop,
        Page::Extensions,
        Page::Apps,
        Page::Accounts,
        Page::Accessibility,
        Page::Privacy,
        Page::TimeLanguage,
        Page::Compatibility,
        Page::Updates,
        Page::Advanced,
    ];

    /// Left-menu order (differs from `ALL`, whose index is saved).
    pub const SIDEBAR: [Page; 14] = [
        Page::System,
        Page::Bluetooth,
        Page::Network,
        Page::Personalisation,
        Page::Desktop,
        Page::Extensions,
        Page::Apps,
        Page::Accounts,
        Page::Accessibility,
        Page::Privacy,
        Page::TimeLanguage,
        Page::Compatibility,
        Page::Advanced,
        Page::Updates,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::System => "System",
            Page::Bluetooth => "Bluetooth & devices",
            Page::Network => "Network & internet",
            Page::Personalisation => "Personalisation",
            Page::Desktop => "Desktop & windows",
            Page::Extensions => "Extensions",
            Page::Apps => "Apps",
            Page::Accounts => "Accounts",
            Page::Accessibility => "Accessibility",
            Page::Privacy => "Privacy & security",
            Page::TimeLanguage => "Time & language",
            Page::Compatibility => "Compatibility",
            Page::Updates => "Software updates",
            Page::Advanced => "Advanced settings",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Page::System => Icon::System,
            Page::Bluetooth => Icon::Bluetooth,
            Page::Network => Icon::Wifi,
            Page::Personalisation => Icon::Personalise,
            Page::Desktop => Icon::Display,
            Page::Extensions => Icon::Hub,
            Page::Apps => Icon::Apps,
            Page::Accounts => Icon::Accounts,
            Page::Accessibility => Icon::Accessibility,
            Page::Privacy => Icon::Shield,
            Page::TimeLanguage => Icon::Clock,
            Page::Compatibility => Icon::Compatibility,
            Page::Updates => Icon::Update,
            Page::Advanced => Icon::Sparkle,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    Display,
    Sound,
    Notifications,
    Power,
    Storage,
    About,
    Pointer,
    Keyboard,
    DefaultApps,
    /// Network & internet > Advanced network settings (DNS).
    AdvancedNetwork,
    /// Advanced settings > one tweak group (index into `TWEAK_GROUPS`).
    TweakGroup(u8),
    /// Privacy & security > one security page (index into `SECURITY_PAGES`).
    Security(u8),
    /// One section of Desktop & windows (index into `desktop::SECTIONS`,
    /// the last one is Startup apps).
    Desk(u8),
}

impl Detail {
    /// Stable number for saved files (Desk sections are 100 + index).
    pub fn code(self) -> u16 {
        match self {
            Detail::Display => 0,
            Detail::Sound => 1,
            Detail::Notifications => 2,
            Detail::Power => 3,
            Detail::Storage => 4,
            Detail::About => 5,
            Detail::Pointer => 6,
            Detail::Keyboard => 7,
            Detail::DefaultApps => 8,
            Detail::AdvancedNetwork => 10,
            Detail::TweakGroup(i) => 200 + i as u16,
            Detail::Security(i) => 300 + i as u16,
            Detail::Desk(i) => 100 + i as u16,
        }
    }

    /// Own icon (the Desktop & windows sections use their page's).
    pub fn icon(self) -> Option<Icon> {
        Some(match self {
            Detail::Display => Icon::Display,
            Detail::Sound => Icon::Sound,
            Detail::Notifications => Icon::Bell,
            Detail::Power => Icon::Battery,
            Detail::Storage => Icon::Storage,
            Detail::About => Icon::Info,
            Detail::Pointer => Icon::Pointer,
            Detail::Keyboard => Icon::Keyboard,
            Detail::DefaultApps => Icon::Apps,
            Detail::AdvancedNetwork => Icon::Globe,
            Detail::TweakGroup(i) => TWEAK_GROUPS.get(i as usize)?.2,
            Detail::Security(i) => SECURITY_PAGES.get(i as usize)?.1,
            Detail::Desk(_) => return None,
        })
    }

    pub fn from_code(code: u16) -> Option<Detail> {
        Some(match code {
            0 => Detail::Display,
            1 => Detail::Sound,
            2 => Detail::Notifications,
            3 => Detail::Power,
            4 => Detail::Storage,
            5 => Detail::About,
            6 => Detail::Pointer,
            7 => Detail::Keyboard,
            8 => Detail::DefaultApps,
            10 => Detail::AdvancedNetwork,
            200..=206 => Detail::TweakGroup((code - 200) as u8),
            300..=304 => Detail::Security((code - 300) as u8),
            100..=199 => Detail::Desk((code - 100) as u8),
            _ => return None,
        })
    }

    pub fn title(self) -> &'static str {
        match self {
            Detail::Display => "Display",
            Detail::Sound => "Sound",
            Detail::Notifications => "Notifications",
            Detail::Power => "Power & battery",
            Detail::Storage => "Storage",
            Detail::About => "About",
            Detail::Pointer => "Mouse & touchpad",
            Detail::Keyboard => "Keyboard",
            Detail::DefaultApps => "Default applications",
            Detail::AdvancedNetwork => "Advanced network settings",
            Detail::TweakGroup(i) => TWEAK_GROUPS.get(i as usize).map_or("Tweaks", |g| g.1),
            Detail::Security(i) => SECURITY_PAGES.get(i as usize).map_or("Security", |g| g.0),
            Detail::Desk(i) => crate::desktop::section_name(i),
        }
    }
}

/// Security pages under Privacy & security: (title, icon, view key).
pub const SECURITY_PAGES: [(&str, Icon, &str); 5] = [
    ("Security overview", Icon::Shield, "security"),
    ("Firmware updates", Icon::Update, "security-firmware"),
    ("Camera access", Icon::Image, "security-camera"),
    ("Network access", Icon::Wifi, "security-network"),
    ("Virus scan", Icon::Search, "security-virus"),
];

/// Advanced settings groups: (Glint category id, title, icon, view key).
/// Order matches Glint's `TWEAK_CATS` and the Win10 `Cat10::Advanced` list.
pub const TWEAK_GROUPS: [(&str, &str, Icon, &str); 7] = [
    ("privacy", "Privacy", Icon::Shield, "tweaks-privacy"),
    ("performance", "Performance", Icon::Power, "tweaks-performance"),
    ("updates", "Updates", Icon::Update, "tweaks-updates"),
    ("network", "Network", Icon::Globe, "tweaks-network"),
    ("services", "Services", Icon::System, "tweaks-services"),
    ("startup", "Startup", Icon::Clock, "tweaks-startup"),
    ("desktop", "Desktop", Icon::Display, "tweaks-desktop"),
];

impl Page {
    /// Sub-pages shown when the Windows 11 left menu opens this page:
    /// every Windows 10 navigation item that belongs to it, in order.
    pub fn sub_items(self) -> Vec<(&'static str, Icon, View)> {
        let mut out: Vec<(&'static str, Icon, View)> = Vec::new();
        for cat in Cat10::ALL {
            for item in cat.items() {
                if item.2.page() == self && !out.iter().any(|o| o.2 == item.2) {
                    out.push(*item);
                }
            }
        }
        out
    }
}

/// Windows 10 groups the pages into its own ten categories. Each one has a
/// short list of sub-pages shown in the navigation pane, like the real app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat10 {
    System,
    Devices,
    Network,
    Personalisation,
    Desktop,
    Fusion,
    Apps,
    Accounts,
    Time,
    Ease,
    Privacy,
    Advanced,
    Update,
}

impl Cat10 {
    pub const ALL: [Cat10; 13] = [
        Cat10::System,
        Cat10::Devices,
        Cat10::Network,
        Cat10::Personalisation,
        Cat10::Desktop,
        Cat10::Fusion,
        Cat10::Apps,
        Cat10::Accounts,
        Cat10::Time,
        Cat10::Ease,
        Cat10::Privacy,
        Cat10::Advanced,
        Cat10::Update,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Cat10::System => "System",
            Cat10::Devices => "Devices",
            Cat10::Network => "Network & Internet",
            Cat10::Personalisation => "Personalisation",
            Cat10::Desktop => "Desktop & Windows",
            Cat10::Fusion => "Fusion Desk",
            Cat10::Apps => "Apps",
            Cat10::Accounts => "Accounts",
            Cat10::Time => "Time & Language",
            Cat10::Ease => "Ease of Access",
            Cat10::Privacy => "Privacy",
            Cat10::Advanced => "Advanced",
            Cat10::Update => "Update & Security",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Cat10::System => "Display, sound, notifications",
            Cat10::Devices => "Bluetooth, mouse, keyboard",
            Cat10::Network => "Wi-Fi, wired, proxy",
            Cat10::Personalisation => "Background, colours, extensions",
            Cat10::Desktop => "Multitasking, dock, windows, themes",
            Cat10::Fusion => "Panel, taskbar, start menu, glass",
            Cat10::Apps => "Install, defaults, compatibility",
            Cat10::Accounts => "Your info, sign-in options",
            Cat10::Time => "Date, time, language, region",
            Cat10::Ease => "Text size, visual alerts, keys",
            Cat10::Privacy => "Screen lock, location, camera",
            Cat10::Advanced => "Tweaks for privacy, speed, services",
            Cat10::Update => "Software and security updates",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Cat10::System => Icon::Display,
            Cat10::Devices => Icon::Keyboard,
            Cat10::Network => Icon::Globe,
            Cat10::Personalisation => Icon::Personalise,
            Cat10::Desktop => Icon::Display,
            Cat10::Fusion => Icon::Sparkle,
            Cat10::Apps => Icon::Apps,
            Cat10::Accounts => Icon::Accounts,
            Cat10::Time => Icon::Clock,
            Cat10::Ease => Icon::Accessibility,
            Cat10::Privacy => Icon::Shield,
            Cat10::Advanced => Icon::Sparkle,
            Cat10::Update => Icon::Update,
        }
    }

    /// The sub-pages of the category, in navigation-pane order.
    pub fn items(self) -> &'static [(&'static str, Icon, View)] {
        match self {
            Cat10::System => &[
                ("Overview", Icon::System, View::Page(Page::System)),
                (
                    "Display",
                    Icon::Display,
                    View::Detail(Page::System, Detail::Display),
                ),
                (
                    "Sound",
                    Icon::Sound,
                    View::Detail(Page::System, Detail::Sound),
                ),
                (
                    "Notifications & actions",
                    Icon::Bell,
                    View::Detail(Page::System, Detail::Notifications),
                ),
                (
                    "Power & battery",
                    Icon::Battery,
                    View::Detail(Page::System, Detail::Power),
                ),
                (
                    "Storage",
                    Icon::Storage,
                    View::Detail(Page::System, Detail::Storage),
                ),
                (
                    "About",
                    Icon::Info,
                    View::Detail(Page::System, Detail::About),
                ),
            ],
            Cat10::Devices => &[
                (
                    "Bluetooth & other devices",
                    Icon::Bluetooth,
                    View::Page(Page::Bluetooth),
                ),
                (
                    "Mouse & touchpad",
                    Icon::Pointer,
                    View::Detail(Page::Bluetooth, Detail::Pointer),
                ),
                (
                    "Keyboard",
                    Icon::Keyboard,
                    View::Detail(Page::Bluetooth, Detail::Keyboard),
                ),
            ],
            Cat10::Network => &[
                ("Status", Icon::Wifi, View::Page(Page::Network)),
                (
                    "Advanced network settings",
                    Icon::Globe,
                    View::Detail(Page::Network, Detail::AdvancedNetwork),
                ),
            ],
            Cat10::Personalisation => &[
                (
                    "Background & colours",
                    Icon::Personalise,
                    View::Page(Page::Personalisation),
                ),
                ("Extensions", Icon::Hub, View::Page(Page::Extensions)),
            ],
            Cat10::Desktop => &[
                (
                    "Multitasking",
                    Icon::Pointer,
                    View::Detail(Page::Desktop, Detail::Desk(0)),
                ),
                (
                    "Ubuntu Dock",
                    Icon::Apps,
                    View::Detail(Page::Desktop, Detail::Desk(1)),
                ),
                (
                    "Desktop icons",
                    Icon::Image,
                    View::Detail(Page::Desktop, Detail::Desk(2)),
                ),
                (
                    "Windows",
                    Icon::Display,
                    View::Detail(Page::Desktop, Detail::Desk(3)),
                ),
                (
                    "Fonts",
                    Icon::Keyboard,
                    View::Detail(Page::Desktop, Detail::Desk(4)),
                ),
                (
                    "Themes",
                    Icon::Personalise,
                    View::Detail(Page::Desktop, Detail::Desk(5)),
                ),
                (
                    "Mouse, keyboard and sound",
                    Icon::Sound,
                    View::Detail(Page::Desktop, Detail::Desk(6)),
                ),
                (
                    "Startup apps",
                    Icon::Power,
                    View::Detail(Page::Desktop, Detail::Desk(7)),
                ),
            ],
            Cat10::Fusion => &[
                (
                    "Overview",
                    Icon::Sparkle,
                    View::Detail(Page::Desktop, Detail::Desk(8)),
                ),
                (
                    "Panel",
                    Icon::Display,
                    View::Detail(Page::Desktop, Detail::Desk(9)),
                ),
                (
                    "Panel items",
                    Icon::Apps,
                    View::Detail(Page::Desktop, Detail::Desk(10)),
                ),
                (
                    "Taskbar",
                    Icon::Apps,
                    View::Detail(Page::Desktop, Detail::Desk(11)),
                ),
                (
                    "Start menu",
                    Icon::Home,
                    View::Detail(Page::Desktop, Detail::Desk(12)),
                ),
                (
                    "Widgets",
                    Icon::Log,
                    View::Detail(Page::Desktop, Detail::Desk(13)),
                ),
                (
                    "Glass & effects",
                    Icon::Personalise,
                    View::Detail(Page::Desktop, Detail::Desk(14)),
                ),
                (
                    "Desktop",
                    Icon::Storage,
                    View::Detail(Page::Desktop, Detail::Desk(15)),
                ),
            ],
            Cat10::Apps => &[
                ("Apps & features", Icon::Apps, View::Page(Page::Apps)),
                (
                    "Default apps",
                    Icon::Check,
                    View::Detail(Page::Apps, Detail::DefaultApps),
                ),
                (
                    "Compatibility",
                    Icon::Compatibility,
                    View::Page(Page::Compatibility),
                ),
            ],
            Cat10::Accounts => &[("Your info", Icon::Accounts, View::Page(Page::Accounts))],
            Cat10::Time => &[(
                "Date & language",
                Icon::Clock,
                View::Page(Page::TimeLanguage),
            )],
            Cat10::Ease => &[(
                "Vision & input",
                Icon::Accessibility,
                View::Page(Page::Accessibility),
            )],
            Cat10::Privacy => &[
                ("Security & permissions", Icon::Shield, View::Page(Page::Privacy)),
                ("Camera access", Icon::Image, View::Detail(Page::Privacy, Detail::Security(2))),
                ("Network access", Icon::Wifi, View::Detail(Page::Privacy, Detail::Security(3))),
            ],
            Cat10::Advanced => &[
                ("Overview", Icon::Sparkle, View::Page(Page::Advanced)),
                ("Privacy", Icon::Shield, View::Detail(Page::Advanced, Detail::TweakGroup(0))),
                ("Performance", Icon::Power, View::Detail(Page::Advanced, Detail::TweakGroup(1))),
                ("Updates", Icon::Update, View::Detail(Page::Advanced, Detail::TweakGroup(2))),
                ("Network", Icon::Globe, View::Detail(Page::Advanced, Detail::TweakGroup(3))),
                ("Services", Icon::System, View::Detail(Page::Advanced, Detail::TweakGroup(4))),
                ("Startup", Icon::Clock, View::Detail(Page::Advanced, Detail::TweakGroup(5))),
                ("Desktop", Icon::Display, View::Detail(Page::Advanced, Detail::TweakGroup(6))),
            ],
            Cat10::Update => &[
                ("Software Update", Icon::Update, View::Page(Page::Updates)),
                ("Security", Icon::Shield, View::Detail(Page::Privacy, Detail::Security(0))),
                ("Firmware updates", Icon::Update, View::Detail(Page::Privacy, Detail::Security(1))),
                ("Virus scan", Icon::Search, View::Detail(Page::Privacy, Detail::Security(4))),
            ],
        }
    }

    /// The category a view belongs to (none for the home screen).
    pub fn of(view: View) -> Option<Cat10> {
        Some(match view {
            View::Home => return None,
            View::Detail(_, Detail::Pointer | Detail::Keyboard) => Cat10::Devices,
            View::Detail(_, Detail::DefaultApps) => Cat10::Apps,
            View::Detail(_, Detail::AdvancedNetwork) => Cat10::Network,
            View::Detail(_, Detail::TweakGroup(_)) => Cat10::Advanced,
            View::Detail(_, Detail::Security(2 | 3)) => Cat10::Privacy,
            View::Detail(_, Detail::Security(_)) => Cat10::Update,
            View::Detail(_, Detail::Desk(i)) if i >= 8 => Cat10::Fusion,
            View::Detail(_, Detail::Desk(_)) => Cat10::Desktop,
            View::Detail(_, _) => Cat10::System,
            View::Page(Page::System) => Cat10::System,
            View::Page(Page::Bluetooth) => Cat10::Devices,
            View::Page(Page::Network) => Cat10::Network,
            View::Page(Page::Personalisation | Page::Extensions) => Cat10::Personalisation,
            View::Page(Page::Desktop) => Cat10::Desktop,
            View::Page(Page::Apps | Page::Compatibility) => Cat10::Apps,
            View::Page(Page::Accounts) => Cat10::Accounts,
            View::Page(Page::TimeLanguage) => Cat10::Time,
            View::Page(Page::Accessibility) => Cat10::Ease,
            View::Page(Page::Privacy) => Cat10::Privacy,
            View::Page(Page::Updates) => Cat10::Update,
            View::Page(Page::Advanced) => Cat10::Advanced,
        })
    }

    /// Whether a navigation item stands for the current view.
    pub fn same(item: View, current: View) -> bool {
        match (item, current) {
            (View::Detail(_, a), View::Detail(_, b)) => a == b,
            (a, b) => a == b,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    /// The Windows 10 home screen (category tiles). Windows 11 opens System.
    Home,
    Page(Page),
    Detail(Page, Detail),
}

impl View {
    /// Name shown in Recently accessed: the sub-page or the page.
    pub fn title(self) -> &'static str {
        match self {
            View::Detail(_, d) => d.title(),
            v => v.page().label(),
        }
    }

    /// Icon of the sub-page, else the page's icon.
    pub fn icon(self) -> Icon {
        match self {
            View::Detail(p, d) => d.icon().unwrap_or(p.icon()),
            v => v.page().icon(),
        }
    }

    /// Text form for the recently accessed file: `page N` or `detail N D`
    /// (N = index in `Page::ALL`, D = `Detail::code`). Home has none.
    pub fn key(self) -> Option<String> {
        let index = |p: Page| Page::ALL.iter().position(|x| *x == p);
        match self {
            View::Home => None,
            View::Page(p) => Some(format!("page {}", index(p)?)),
            View::Detail(p, d) => Some(format!("detail {} {}", index(p)?, d.code())),
        }
    }

    pub fn from_key(text: &str) -> Option<View> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let page = |w: &str| Page::ALL.get(w.parse::<usize>().ok()?).copied();
        match words.as_slice() {
            ["page", n] => Some(View::Page(page(n)?)),
            ["detail", n, d] => Some(View::Detail(page(n)?, Detail::from_code(d.parse().ok()?)?)),
            _ => None,
        }
    }

    pub fn page(self) -> Page {
        match self {
            View::Home => Page::System,
            View::Page(p) => p,
            View::Detail(p, _) => p,
        }
    }

    pub(crate) fn salt(self) -> &'static str {
        match self {
            View::Home => "home",
            View::Page(Page::System) => "system",
            View::Page(Page::Bluetooth) => "bluetooth",
            View::Page(Page::Network) => "network",
            View::Page(Page::Personalisation) => "personalisation",
            View::Page(Page::Desktop) => "desktop",
            View::Page(Page::Extensions) => "extensions",
            View::Page(Page::Apps) => "apps",
            View::Page(Page::Accounts) => "accounts",
            View::Page(Page::Accessibility) => "accessibility",
            View::Page(Page::Privacy) => "privacy",
            View::Page(Page::TimeLanguage) => "time",
            View::Page(Page::Compatibility) => "compat",
            View::Page(Page::Updates) => "updates",
            View::Page(Page::Advanced) => "advanced",
            View::Detail(_, Detail::Display) => "display",
            View::Detail(_, Detail::Sound) => "sound",
            View::Detail(_, Detail::Notifications) => "notifications",
            View::Detail(_, Detail::Power) => "power",
            View::Detail(_, Detail::Storage) => "storage",
            View::Detail(_, Detail::About) => "about",
            View::Detail(_, Detail::Pointer) => "pointer",
            View::Detail(_, Detail::Keyboard) => "keyboard",
            View::Detail(_, Detail::DefaultApps) => "default-apps",
            View::Detail(_, Detail::AdvancedNetwork) => "advanced-network",
            View::Detail(_, Detail::TweakGroup(i)) => {
                TWEAK_GROUPS.get(i as usize).map_or("tweaks", |g| g.3)
            }
            View::Detail(_, Detail::Security(i)) => {
                SECURITY_PAGES.get(i as usize).map_or("security", |g| g.2)
            }
            View::Detail(_, Detail::Desk(i)) => crate::desktop::section_key(i),
        }
    }
}
