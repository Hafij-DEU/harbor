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
}

impl Page {
    pub const ALL: [Page; 13] = [
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
    /// One section of Desktop & windows (index into `desktop::SECTIONS`,
    /// the last one is Startup apps).
    Desk(u8),
}

impl Detail {
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
            Detail::Desk(i) => crate::desktop::section_name(i),
        }
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
    Update,
}

impl Cat10 {
    pub const ALL: [Cat10; 12] = [
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
            Cat10::Network => &[("Status", Icon::Wifi, View::Page(Page::Network))],
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
            Cat10::Privacy => &[(
                "Security & permissions",
                Icon::Shield,
                View::Page(Page::Privacy),
            )],
            Cat10::Update => &[("Software Update", Icon::Update, View::Page(Page::Updates))],
        }
    }

    /// The category a view belongs to (none for the home screen).
    pub fn of(view: View) -> Option<Cat10> {
        Some(match view {
            View::Home => return None,
            View::Detail(_, Detail::Pointer | Detail::Keyboard) => Cat10::Devices,
            View::Detail(_, Detail::DefaultApps) => Cat10::Apps,
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
            View::Detail(_, Detail::Display) => "display",
            View::Detail(_, Detail::Sound) => "sound",
            View::Detail(_, Detail::Notifications) => "notifications",
            View::Detail(_, Detail::Power) => "power",
            View::Detail(_, Detail::Storage) => "storage",
            View::Detail(_, Detail::About) => "about",
            View::Detail(_, Detail::Pointer) => "pointer",
            View::Detail(_, Detail::Keyboard) => "keyboard",
            View::Detail(_, Detail::DefaultApps) => "default-apps",
            View::Detail(_, Detail::Desk(i)) => crate::desktop::section_key(i),
        }
    }
}
