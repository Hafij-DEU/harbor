//! "Desktop & windows" (the Ubuntu Settings and GNOME Tweaks options that the
//! other pages do not cover) and "Extensions". Everything is written straight
//! to gsettings, the autostart folder or the shell's D-Bus API, so neither
//! GNOME Settings nor Tweaks nor the Extensions app is needed. Rows whose
//! schema is not installed (no Ubuntu Dock, no desktop icons) hide themselves.

use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Context, RichText, Ui};

use crate::app::App;
use crate::icons::Icon;
use crate::pages::{combo_row, range_row};
use crate::theme::regular;
use crate::widgets::{self, Right, ROW_PAD};

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    /// Switch shown as the opposite of the stored value.
    BoolInverted,
    Choice(&'static [(&'static str, &'static str)]),
    Int(i64, i64, i64),
    Font,
    Theme(ThemeKind),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemeKind {
    Gtk,
    Icons,
    Cursor,
}

struct Item {
    section: &'static str,
    icon: Icon,
    title: &'static str,
    sub: &'static str,
    schema: &'static str,
    key: &'static str,
    kind: Kind,
}

const TITLE_ACTIONS: &[(&str, &str)] = &[
    ("toggle-maximize", "Maximize"),
    ("minimize", "Minimize"),
    ("lower", "Send to back"),
    ("menu", "Window menu"),
    ("none", "Nothing"),
];

/// Section names, then "Startup apps" (index 7). Each one is a page of its
/// own in the Windows 10 sidebar.
pub fn section_name(index: u8) -> &'static str {
    match index as usize {
        i if i < SECTIONS.len() => SECTIONS[i],
        FUSION_INDEX => "Fusion Desk",
        i if i > FUSION_INDEX && i <= FUSION_INDEX + FUSION_GROUPS.len() => {
            FUSION_GROUPS[i - FUSION_INDEX - 1].trim_start_matches("Fusion Desk · ")
        }
        _ => "Startup apps",
    }
}

/// Sidebar index of the Fusion Desk page (after Startup apps).
pub const FUSION_INDEX: usize = SECTIONS.len() + 1;

pub fn section_key(index: u8) -> &'static str {
    const KEYS: [&str; 16] = [
        "desk-multitasking",
        "desk-dock",
        "desk-icons",
        "desk-windows",
        "desk-fonts",
        "desk-themes",
        "desk-input",
        "desk-startup",
        "desk-fusion",
        "fusion-panel",
        "fusion-items",
        "fusion-taskbar",
        "fusion-menu",
        "fusion-widgets",
        "fusion-glass",
        "fusion-desktop",
    ];
    KEYS.get(index as usize).copied().unwrap_or("desk-startup")
}

/// One section of Desktop & windows on its own page.
pub fn desktop_section(app: &mut App, ui: &mut Ui, index: u8) {
    show(app, ui, Some(index as usize));
}

pub fn desktop_page(app: &mut App, ui: &mut Ui) {
    show(app, ui, None);
}

const FUSION_SCHEMA: &str = "org.gnome.shell.extensions.fusion-desk";
const FUSION_UUID: &str = "fusion-desk@hafij-deu.github.io";
const FUSION_REPO: &str = "Hafij-DEU/Fusion-desk-gnome-extension";
/// Fusion Desk groups, in page order.
const FUSION_GROUPS: [&str; 7] = [
    "Fusion Desk · Panel",
    "Fusion Desk · Panel items",
    "Fusion Desk · Taskbar",
    "Fusion Desk · Start menu",
    "Fusion Desk · Widgets",
    "Fusion Desk · Glass & effects",
    "Fusion Desk · Desktop",
];

const SECTIONS: [&str; 7] = [
    "Multitasking",
    "Ubuntu Dock",
    "Desktop icons",
    "Windows",
    "Fonts",
    "Themes",
    "Mouse, keyboard and sound",
];

const ITEMS: &[Item] = &[
    // Multitasking (Ubuntu Settings)
    Item {
        section: "Multitasking",
        icon: Icon::Pointer,
        title: "Hot corner",
        sub: "Touch the top-left corner to open the overview",
        schema: "org.gnome.desktop.interface",
        key: "enable-hot-corners",
        kind: Kind::Bool,
    },
    Item {
        section: "Multitasking",
        icon: Icon::Display,
        title: "Active screen edges",
        sub: "Drag windows to the screen edges to tile them",
        schema: "org.gnome.mutter",
        key: "edge-tiling",
        kind: Kind::Bool,
    },
    Item {
        section: "Multitasking",
        icon: Icon::Apps,
        title: "Dynamic workspaces",
        sub: "Add and remove workspaces as you need them",
        schema: "org.gnome.mutter",
        key: "dynamic-workspaces",
        kind: Kind::Bool,
    },
    Item {
        section: "Multitasking",
        icon: Icon::Apps,
        title: "Number of workspaces",
        sub: "Used when dynamic workspaces are off",
        schema: "org.gnome.desktop.wm.preferences",
        key: "num-workspaces",
        kind: Kind::Int(1, 12, 1),
    },
    Item {
        section: "Multitasking",
        icon: Icon::Display,
        title: "Workspaces on the main display only",
        sub: "Other displays keep their windows when you switch",
        schema: "org.gnome.mutter",
        key: "workspaces-only-on-primary",
        kind: Kind::Bool,
    },
    Item {
        section: "Multitasking",
        icon: Icon::Apps,
        title: "Alt+Tab shows this workspace only",
        sub: "Leave off to switch between apps on every workspace",
        schema: "org.gnome.shell.app-switcher",
        key: "current-workspace-only",
        kind: Kind::Bool,
    },
    // Ubuntu Dock
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Apps,
        title: "Auto-hide the dock",
        sub: "The dock hides when a window touches it",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "dock-fixed",
        kind: Kind::BoolInverted,
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Display,
        title: "Panel mode",
        sub: "Stretch the dock across the screen edge",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "extend-height",
        kind: Kind::Bool,
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Apps,
        title: "Icon size",
        sub: "Largest size of dock icons, in pixels",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "dash-max-icon-size",
        kind: Kind::Int(16, 64, 2),
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Display,
        title: "Position on screen",
        sub: "Edge of the screen the dock sits on",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "dock-position",
        kind: Kind::Choice(&[("LEFT", "Left"), ("BOTTOM", "Bottom"), ("RIGHT", "Right")]),
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Pointer,
        title: "Click action",
        sub: "What clicking a running app does",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "click-action",
        kind: Kind::Choice(&[
            ("focus-or-previews", "Focus or show previews"),
            ("minimize", "Minimize"),
            ("previews", "Show previews"),
            ("cycle-windows", "Cycle windows"),
            ("focus-minimize-or-previews", "Focus, minimize or previews"),
        ]),
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Display,
        title: "Show on all displays",
        sub: "One dock on every monitor",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "multi-monitor",
        kind: Kind::Bool,
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Trash,
        title: "Show Trash",
        sub: "Trash icon in the dock",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "show-trash",
        kind: Kind::Bool,
    },
    Item {
        section: "Ubuntu Dock",
        icon: Icon::Storage,
        title: "Show volumes and devices",
        sub: "Mounted drives in the dock",
        schema: "org.gnome.shell.extensions.dash-to-dock",
        key: "show-mounts",
        kind: Kind::Bool,
    },
    // Desktop icons
    Item {
        section: "Desktop icons",
        icon: Icon::Storage,
        title: "Show the Home folder",
        sub: "Home icon on the desktop",
        schema: "org.gnome.shell.extensions.ding",
        key: "show-home",
        kind: Kind::Bool,
    },
    Item {
        section: "Desktop icons",
        icon: Icon::Trash,
        title: "Show Trash",
        sub: "Trash icon on the desktop",
        schema: "org.gnome.shell.extensions.ding",
        key: "show-trash",
        kind: Kind::Bool,
    },
    Item {
        section: "Desktop icons",
        icon: Icon::Image,
        title: "Icon size",
        sub: "Size of desktop icons",
        schema: "org.gnome.shell.extensions.ding",
        key: "icon-size",
        kind: Kind::Choice(&[
            ("tiny", "Tiny"),
            ("small", "Small"),
            ("standard", "Standard"),
            ("large", "Large"),
        ]),
    },
    Item {
        section: "Desktop icons",
        icon: Icon::Display,
        title: "New icons start at",
        sub: "Corner where new desktop icons appear",
        schema: "org.gnome.shell.extensions.ding",
        key: "start-corner",
        kind: Kind::Choice(&[
            ("top-left", "Top left"),
            ("top-right", "Top right"),
            ("bottom-left", "Bottom left"),
            ("bottom-right", "Bottom right"),
        ]),
    },
    // Windows (Tweaks)
    Item {
        section: "Windows",
        icon: Icon::Apps,
        title: "Title bar buttons",
        sub: "Buttons and their side",
        schema: "org.gnome.desktop.wm.preferences",
        key: "button-layout",
        kind: Kind::Choice(&[
            (
                "appmenu:minimize,maximize,close",
                "Minimize, maximize, close (right)",
            ),
            ("appmenu:close", "Close only (right)"),
            ("appmenu:minimize,close", "Minimize, close (right)"),
            (
                "close,minimize,maximize:appmenu",
                "Close, minimize, maximize (left)",
            ),
        ]),
    },
    Item {
        section: "Windows",
        icon: Icon::Pointer,
        title: "Double-click the title bar",
        sub: "Action on a title bar double-click",
        schema: "org.gnome.desktop.wm.preferences",
        key: "action-double-click-titlebar",
        kind: Kind::Choice(TITLE_ACTIONS),
    },
    Item {
        section: "Windows",
        icon: Icon::Pointer,
        title: "Middle-click the title bar",
        sub: "Action on a title bar middle click",
        schema: "org.gnome.desktop.wm.preferences",
        key: "action-middle-click-titlebar",
        kind: Kind::Choice(TITLE_ACTIONS),
    },
    Item {
        section: "Windows",
        icon: Icon::Pointer,
        title: "Right-click the title bar",
        sub: "Action on a title bar right click",
        schema: "org.gnome.desktop.wm.preferences",
        key: "action-right-click-titlebar",
        kind: Kind::Choice(TITLE_ACTIONS),
    },
    Item {
        section: "Windows",
        icon: Icon::Pointer,
        title: "Window focus",
        sub: "How a window becomes active",
        schema: "org.gnome.desktop.wm.preferences",
        key: "focus-mode",
        kind: Kind::Choice(&[
            ("click", "Click to focus"),
            ("sloppy", "Focus on hover"),
            ("mouse", "Focus follows mouse"),
        ]),
    },
    Item {
        section: "Windows",
        icon: Icon::Display,
        title: "Centre new windows",
        sub: "Open windows in the middle of the screen",
        schema: "org.gnome.mutter",
        key: "center-new-windows",
        kind: Kind::Bool,
    },
    Item {
        section: "Windows",
        icon: Icon::Apps,
        title: "Attach dialogs to their window",
        sub: "Dialogs move with the window that opened them",
        schema: "org.gnome.mutter",
        key: "attach-modal-dialogs",
        kind: Kind::Bool,
    },
    Item {
        section: "Windows",
        icon: Icon::Keyboard,
        title: "Window action key",
        sub: "Hold it and drag to move a window anywhere",
        schema: "org.gnome.desktop.wm.preferences",
        key: "mouse-button-modifier",
        kind: Kind::Choice(&[("<Super>", "Super"), ("<Alt>", "Alt"), ("", "Off")]),
    },
    Item {
        section: "Windows",
        icon: Icon::Pointer,
        title: "Resize with the right button",
        sub: "Action key + right drag resizes a window",
        schema: "org.gnome.desktop.wm.preferences",
        key: "resize-with-right-button",
        kind: Kind::Bool,
    },
    // Fonts
    Item {
        section: "Fonts",
        icon: Icon::Keyboard,
        title: "Interface text",
        sub: "Menus, buttons and labels",
        schema: "org.gnome.desktop.interface",
        key: "font-name",
        kind: Kind::Font,
    },
    Item {
        section: "Fonts",
        icon: Icon::Keyboard,
        title: "Document text",
        sub: "Text in documents",
        schema: "org.gnome.desktop.interface",
        key: "document-font-name",
        kind: Kind::Font,
    },
    Item {
        section: "Fonts",
        icon: Icon::Keyboard,
        title: "Monospace text",
        sub: "Terminals and code",
        schema: "org.gnome.desktop.interface",
        key: "monospace-font-name",
        kind: Kind::Font,
    },
    Item {
        section: "Fonts",
        icon: Icon::Display,
        title: "Hinting",
        sub: "Fit letters to the pixel grid",
        schema: "org.gnome.desktop.interface",
        key: "font-hinting",
        kind: Kind::Choice(&[
            ("none", "None"),
            ("slight", "Slight"),
            ("medium", "Medium"),
            ("full", "Full"),
        ]),
    },
    Item {
        section: "Fonts",
        icon: Icon::Display,
        title: "Antialiasing",
        sub: "Smooth font edges",
        schema: "org.gnome.desktop.interface",
        key: "font-antialiasing",
        kind: Kind::Choice(&[
            ("grayscale", "Standard (greyscale)"),
            ("rgba", "Subpixel (for LCD screens)"),
            ("none", "None"),
        ]),
    },
    // Themes
    Item {
        section: "Themes",
        icon: Icon::Personalise,
        title: "Legacy apps",
        sub: "Theme of GTK 3 applications",
        schema: "org.gnome.desktop.interface",
        key: "gtk-theme",
        kind: Kind::Theme(ThemeKind::Gtk),
    },
    Item {
        section: "Themes",
        icon: Icon::Image,
        title: "Icons",
        sub: "Icon theme of the desktop and apps",
        schema: "org.gnome.desktop.interface",
        key: "icon-theme",
        kind: Kind::Theme(ThemeKind::Icons),
    },
    Item {
        section: "Themes",
        icon: Icon::Pointer,
        title: "Cursor",
        sub: "Mouse pointer theme",
        schema: "org.gnome.desktop.interface",
        key: "cursor-theme",
        kind: Kind::Theme(ThemeKind::Cursor),
    },
    // Mouse, keyboard and sound (Tweaks)
    Item {
        section: "Mouse, keyboard and sound",
        icon: Icon::Keyboard,
        title: "Overview key",
        sub: "Key that opens the overview",
        schema: "org.gnome.mutter",
        key: "overlay-key",
        kind: Kind::Choice(&[("Super_L", "Left Super"), ("Super_R", "Right Super")]),
    },
    Item {
        section: "Mouse, keyboard and sound",
        icon: Icon::Pointer,
        title: "Middle-click paste",
        sub: "Paste the selected text with the middle button",
        schema: "org.gnome.desktop.interface",
        key: "gtk-enable-primary-paste",
        kind: Kind::Bool,
    },
    Item {
        section: "Mouse, keyboard and sound",
        icon: Icon::Pointer,
        title: "Mouse acceleration",
        sub: "How the pointer speeds up",
        schema: "org.gnome.desktop.peripherals.mouse",
        key: "accel-profile",
        kind: Kind::Choice(&[
            ("default", "Default"),
            ("adaptive", "Adaptive"),
            ("flat", "Flat (none)"),
        ]),
    },
    Item {
        section: "Mouse, keyboard and sound",
        icon: Icon::Sound,
        title: "Over-amplification",
        sub: "Allow volume above 100%",
        schema: "org.gnome.desktop.sound",
        key: "allow-volume-above-100-percent",
        kind: Kind::Bool,
    },
    // Fusion Desk (only shown when the extension is installed)
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Display,
        title: "Panel position",
        sub: "Edge of the screen the panel sits on",
        schema: FUSION_SCHEMA,
        key: "panel-position",
        kind: Kind::Choice(&[("BOTTOM", "Bottom"), ("TOP", "Top")]),
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Display,
        title: "Panel height",
        sub: "Height in pixels",
        schema: FUSION_SCHEMA,
        key: "panel-height",
        kind: Kind::Int(24, 96, 2),
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Sparkle,
        title: "Panel opacity",
        sub: "Background opacity in percent",
        schema: FUSION_SCHEMA,
        key: "panel-opacity",
        kind: Kind::Int(0, 100, 5),
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Apps,
        title: "Item style",
        sub: "Rounded pills or flat items",
        schema: FUSION_SCHEMA,
        key: "panel-style",
        kind: Kind::Choice(&[("PILLS", "Pills"), ("FLAT", "Flat")]),
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Moon,
        title: "Panel colours",
        sub: "Follow light/dark or keep one",
        schema: FUSION_SCHEMA,
        key: "panel-theme",
        kind: Kind::Choice(&[
            ("AUTO", "Follow system"),
            ("DARK", "Always dark"),
            ("LIGHT", "Always light"),
        ]),
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Display,
        title: "Floating taskbar",
        sub: "Rounded, with a gap from the screen edges",
        schema: FUSION_SCHEMA,
        key: "deepin-floating-taskbar",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Display,
        title: "Panel on every monitor",
        sub: "Start button and taskbar on each screen",
        schema: FUSION_SCHEMA,
        key: "multi-monitor",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Pointer,
        title: "Auto-hide the panel",
        sub: "Hide it when windows overlap it",
        schema: FUSION_SCHEMA,
        key: "intellihide",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Pointer,
        title: "Windows that hide it",
        sub: "Which windows make the panel hide",
        schema: FUSION_SCHEMA,
        key: "intellihide-mode",
        kind: Kind::Choice(&[
            ("ALL", "All windows"),
            ("FOCUSED", "Focused window"),
            ("MAXIMIZED", "Maximised windows"),
        ]),
    },
    Item {
        section: "Fusion Desk · Panel",
        icon: Icon::Sparkle,
        title: "Opaque when a window touches it",
        sub: "Dynamic panel opacity",
        schema: FUSION_SCHEMA,
        key: "dynamic-opacity",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Home,
        title: "Start button",
        sub: "The start menu button",
        schema: FUSION_SCHEMA,
        key: "menu-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Apps,
        title: "Show Applications button",
        sub: "Opens the app grid",
        schema: FUSION_SCHEMA,
        key: "apps-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::System,
        title: "Settings on the taskbar",
        sub: "Harbor Settings after the apps button",
        schema: FUSION_SCHEMA,
        key: "settings-button",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Search,
        title: "Search button",
        sub: "After the start button",
        schema: FUSION_SCHEMA,
        key: "windows-search",
        kind: Kind::Choice(&[
            ("BOX", "Search box"),
            ("ICON", "Icon only"),
            ("HIDDEN", "Hidden"),
        ]),
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Apps,
        title: "Task View button",
        sub: "Opens the overview",
        schema: FUSION_SCHEMA,
        key: "windows-task-view",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Display,
        title: "Activities button",
        sub: "GNOME's Activities button",
        schema: FUSION_SCHEMA,
        key: "activities-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Info,
        title: "Window title",
        sub: "Title of the focused window",
        schema: FUSION_SCHEMA,
        key: "title-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Apps,
        title: "Workspace switcher",
        sub: "Workspace indicator",
        schema: FUSION_SCHEMA,
        key: "workspaces-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Sound,
        title: "Media controls",
        sub: "Play, pause and track title",
        schema: FUSION_SCHEMA,
        key: "media-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Log,
        title: "System monitor",
        sub: "CPU, RAM, network and temperature",
        schema: FUSION_SCHEMA,
        key: "sysmon-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Copy,
        title: "Clipboard history icon",
        sub: "Clipboard history on the panel",
        schema: FUSION_SCHEMA,
        key: "clipboard-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Image,
        title: "Screenshot button",
        sub: "Take a screenshot",
        schema: FUSION_SCHEMA,
        key: "screenshot-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Hub,
        title: "App Hub button",
        sub: "Opens App Hub",
        schema: FUSION_SCHEMA,
        key: "apphub-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::ChevronRight,
        title: "Hidden tray icons arrow",
        sub: "Other apps' tray icons behind an arrow",
        schema: FUSION_SCHEMA,
        key: "overflow-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Display,
        title: "Show Desktop strip",
        sub: "Minimise all windows",
        schema: FUSION_SCHEMA,
        key: "desktop-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Battery,
        title: "Battery as its own button",
        sub: "Battery next to the system menu",
        schema: FUSION_SCHEMA,
        key: "battery-standalone",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Battery,
        title: "Battery look",
        sub: "Coloured fill or GNOME icon",
        schema: FUSION_SCHEMA,
        key: "battery-style",
        kind: Kind::Choice(&[("COLOR", "Coloured"), ("GNOME", "GNOME")]),
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Bell,
        title: "Control centre",
        sub: "Side panel with toggles and notifications",
        schema: FUSION_SCHEMA,
        key: "deepin-control-button",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Panel items",
        icon: Icon::Shield,
        title: "Hide duplicates",
        sub: "Hide items other extensions already show",
        schema: FUSION_SCHEMA,
        key: "avoid-duplicates",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Apps,
        title: "Taskbar",
        sub: "Pinned and running apps",
        schema: FUSION_SCHEMA,
        key: "taskbar-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Apps,
        title: "Icon size",
        sub: "Taskbar icon size in pixels",
        schema: FUSION_SCHEMA,
        key: "taskbar-icon-size",
        kind: Kind::Int(16, 64, 2),
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Check,
        title: "Pinned apps",
        sub: "Show pinned apps",
        schema: FUSION_SCHEMA,
        key: "taskbar-show-favorites",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Apps,
        title: "Group windows",
        sub: "One button per app",
        schema: FUSION_SCHEMA,
        key: "taskbar-group-apps",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Info,
        title: "Window titles",
        sub: "Titles next to running app icons",
        schema: FUSION_SCHEMA,
        key: "taskbar-labels",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Display,
        title: "Current workspace only",
        sub: "Only windows from this workspace",
        schema: FUSION_SCHEMA,
        key: "taskbar-isolate-workspaces",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Image,
        title: "Window previews",
        sub: "Previews on hover",
        schema: FUSION_SCHEMA,
        key: "taskbar-previews",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Sparkle,
        title: "Running indicator",
        sub: "Mark under running apps",
        schema: FUSION_SCHEMA,
        key: "taskbar-indicator-style",
        kind: Kind::Choice(&[
            ("DOTS", "Dots"),
            ("DASHES", "Dashes"),
            ("SQUARES", "Squares"),
            ("LINE", "Line"),
            ("NONE", "None"),
        ]),
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Sparkle,
        title: "Indicator position",
        sub: "Edge of the icon",
        schema: FUSION_SCHEMA,
        key: "taskbar-indicator-position",
        kind: Kind::Choice(&[("BOTTOM", "Bottom"), ("TOP", "Top")]),
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Pointer,
        title: "Left click",
        sub: "What clicking an app does",
        schema: FUSION_SCHEMA,
        key: "taskbar-click-action",
        kind: Kind::Choice(&[
            ("CYCLE", "Raise, minimise, cycle"),
            ("MINIMIZE", "Raise or minimise all"),
            ("PREVIEW", "Show previews"),
        ]),
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Pointer,
        title: "Middle click",
        sub: "Middle click on an app",
        schema: FUSION_SCHEMA,
        key: "taskbar-middle-click",
        kind: Kind::Choice(&[
            ("NEW_WINDOW", "New window"),
            ("MINIMIZE", "Minimise"),
            ("QUIT", "Quit"),
        ]),
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Pointer,
        title: "Scroll on an icon",
        sub: "Scroll wheel over an app",
        schema: FUSION_SCHEMA,
        key: "taskbar-scroll-action",
        kind: Kind::Choice(&[
            ("CYCLE", "Cycle windows"),
            ("WORKSPACE", "Switch workspace"),
            ("NOTHING", "Nothing"),
        ]),
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Keyboard,
        title: "Super+1…9",
        sub: "Open taskbar apps by number",
        schema: FUSION_SCHEMA,
        key: "taskbar-hotkeys",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Taskbar",
        icon: Icon::Info,
        title: "Tooltips",
        sub: "App names on hover",
        schema: FUSION_SCHEMA,
        key: "taskbar-tooltips",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Home,
        title: "Layout",
        sub: "Start menu layout",
        schema: FUSION_SCHEMA,
        key: "menu-layout",
        kind: Kind::Choice(&[
            ("eleven", "Modern"),
            ("grid", "Launcher Grid"),
            ("runner", "Runner"),
            ("windows", "Classic"),
            ("fusion", "Fusion"),
            ("mint", "Sidebar"),
            ("plasma", "Tabbed"),
        ]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Keyboard,
        title: "Super key opens",
        sub: "What the Super key does",
        schema: FUSION_SCHEMA,
        key: "super-key",
        kind: Kind::Choice(&[
            ("MENU", "Start menu"),
            ("RUNNER", "Runner"),
            ("LAUNCHER", "App launcher"),
            ("OVERVIEW", "Overview"),
        ]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Apps,
        title: "App launcher",
        sub: "Start menu, launcher or both",
        schema: FUSION_SCHEMA,
        key: "deepin-start",
        kind: Kind::Choice(&[
            ("FUSION", "Start menu only"),
            ("BOTH", "Menu and launcher"),
            ("DEEPIN", "Start opens launcher"),
        ]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Display,
        title: "Launcher size",
        sub: "Fullscreen grid or small window",
        schema: FUSION_SCHEMA,
        key: "deepin-launcher-mode",
        kind: Kind::Choice(&[("FULLSCREEN", "Fullscreen"), ("WINDOWED", "Window")]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Display,
        title: "Menu opens",
        sub: "Where the menu appears",
        schema: FUSION_SCHEMA,
        key: "menu-position",
        kind: Kind::Choice(&[("BUTTON", "At the button"), ("CENTER", "Centre")]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Apps,
        title: "First view",
        sub: "Shown when the menu opens",
        schema: FUSION_SCHEMA,
        key: "menu-default-view",
        kind: Kind::Choice(&[("PINNED", "Pinned"), ("ALL", "All apps")]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Check,
        title: "Pinned apps from",
        sub: "Taskbar favourites or own list",
        schema: FUSION_SCHEMA,
        key: "menu-pinned-source",
        kind: Kind::Choice(&[("OWN", "Own list"), ("FAVORITES", "Taskbar favourites")]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Home,
        title: "Start button",
        sub: "Icon, text or both",
        schema: FUSION_SCHEMA,
        key: "menu-button-style",
        kind: Kind::Choice(&[
            ("ICON", "Icon"),
            ("TEXT", "Text"),
            ("ICON_TEXT", "Icon and text"),
        ]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Home,
        title: "Start button icon size",
        sub: "In pixels",
        schema: FUSION_SCHEMA,
        key: "menu-button-icon-size",
        kind: Kind::Int(12, 64, 1),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Info,
        title: "Menu font size",
        sub: "In points, 0 = shell default",
        schema: FUSION_SCHEMA,
        key: "menu-font-size",
        kind: Kind::Int(0, 24, 1),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Apps,
        title: "Tile icon size",
        sub: "App tile icons in pixels",
        schema: FUSION_SCHEMA,
        key: "menu-tile-icon-size",
        kind: Kind::Int(16, 96, 4),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Personalise,
        title: "Menu colours",
        sub: "Shell theme or own colours",
        schema: FUSION_SCHEMA,
        key: "menu-theme",
        kind: Kind::Choice(&[("CUSTOM", "Own colours"), ("SHELL", "Shell theme")]),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Sparkle,
        title: "Corner radius",
        sub: "Menu corners in pixels",
        schema: FUSION_SCHEMA,
        key: "menu-radius",
        kind: Kind::Int(0, 40, 1),
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Accounts,
        title: "User name and picture",
        sub: "At the top of the menu",
        schema: FUSION_SCHEMA,
        key: "menu-show-user",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Storage,
        title: "Places",
        sub: "Home, Documents and more",
        schema: FUSION_SCHEMA,
        key: "menu-show-places",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::System,
        title: "Shortcuts",
        sub: "Settings, Software, Terminal, Files",
        schema: FUSION_SCHEMA,
        key: "menu-show-shortcuts",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Clock,
        title: "Recent files",
        sub: "Recent files category",
        schema: FUSION_SCHEMA,
        key: "menu-show-recent",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Sparkle,
        title: "Frequent apps",
        sub: "Frequently used apps",
        schema: FUSION_SCHEMA,
        key: "menu-show-frequent",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Hub,
        title: "App Hub panel",
        sub: "Updates, installs and disk space",
        schema: FUSION_SCHEMA,
        key: "menu-show-apphub",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Pointer,
        title: "Switch categories on hover",
        sub: "No click needed",
        schema: FUSION_SCHEMA,
        key: "menu-hover-categories",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Search,
        title: "Fuzzy search",
        sub: "Find apps despite typos",
        schema: FUSION_SCHEMA,
        key: "menu-search-fuzzy",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Search,
        title: "Search recent files",
        sub: "Files in search results",
        schema: FUSION_SCHEMA,
        key: "menu-search-files",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Search,
        title: "Calculator",
        sub: "Sums in search",
        schema: FUSION_SCHEMA,
        key: "menu-search-calc",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Start menu",
        icon: Icon::Keyboard,
        title: "Run commands",
        sub: "Type >command",
        schema: FUSION_SCHEMA,
        key: "menu-search-commands",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Log,
        title: "CPU",
        sub: "In the system monitor",
        schema: FUSION_SCHEMA,
        key: "sysmon-cpu",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Log,
        title: "RAM",
        sub: "In the system monitor",
        schema: FUSION_SCHEMA,
        key: "sysmon-ram",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Wifi,
        title: "Network speed",
        sub: "In the system monitor",
        schema: FUSION_SCHEMA,
        key: "sysmon-net",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Sun,
        title: "CPU temperature",
        sub: "In the system monitor",
        schema: FUSION_SCHEMA,
        key: "sysmon-temp",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Clock,
        title: "Monitor refresh",
        sub: "Seconds between updates",
        schema: FUSION_SCHEMA,
        key: "sysmon-interval",
        kind: Kind::Int(1, 10, 1),
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Keyboard,
        title: "Super+V opens the clipboard",
        sub: "Clipboard history shortcut",
        schema: FUSION_SCHEMA,
        key: "clipboard-super-v",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Copy,
        title: "Paste on select",
        sub: "Paste the chosen entry straight away",
        schema: FUSION_SCHEMA,
        key: "clipboard-auto-paste",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Image,
        title: "Keep copied images",
        sub: "Images in the history",
        schema: FUSION_SCHEMA,
        key: "clipboard-images",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Storage,
        title: "Save history",
        sub: "Keep it after a restart",
        schema: FUSION_SCHEMA,
        key: "clipboard-persist",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Copy,
        title: "History size",
        sub: "Entries to keep",
        schema: FUSION_SCHEMA,
        key: "clipboard-size",
        kind: Kind::Int(5, 200, 5),
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Power,
        title: "Caffeine",
        sub: "Keep awake in Quick Settings",
        schema: FUSION_SCHEMA,
        key: "caffeine-visible",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Sound,
        title: "Apps without media controls",
        sub: "Also show apps that only play audio",
        schema: FUSION_SCHEMA,
        key: "media-audio-fallback",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Widgets",
        icon: Icon::Clock,
        title: "Custom clock format",
        sub: "Use Fusion Desk's clock format",
        schema: FUSION_SCHEMA,
        key: "clock-custom",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Sparkle,
        title: "Glass look",
        sub: "Taskbar, menus and popups",
        schema: FUSION_SCHEMA,
        key: "glass-effect",
        kind: Kind::Choice(&[("OFF", "Off"), ("SOFT", "Soft"), ("STRONG", "Strong")]),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Sparkle,
        title: "Glass variant",
        sub: "Style of the glass",
        schema: FUSION_SCHEMA,
        key: "glass-style",
        kind: Kind::Choice(&[
            ("CLEAR", "Clear"),
            ("FROSTED", "Frosted"),
            ("GEL", "Gel"),
            ("TINTED", "Tinted"),
            ("SMOKE", "Smoke"),
            ("HALO", "Halo"),
            ("GLOW", "Glow"),
        ]),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Sparkle,
        title: "Material",
        sub: "Subtle shine or strong gloss",
        schema: FUSION_SCHEMA,
        key: "glass-material",
        kind: Kind::Choice(&[("GLASS", "Glass"), ("GLOSS", "Gloss")]),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Sparkle,
        title: "Glass opacity",
        sub: "Menus and panels, percent",
        schema: FUSION_SCHEMA,
        key: "glass-opacity",
        kind: Kind::Int(10, 100, 5),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Sparkle,
        title: "Light rim",
        sub: "Thin bevel along the edges",
        schema: FUSION_SCHEMA,
        key: "glass-rim",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Sun,
        title: "Glare",
        sub: "Reflections in the corners",
        schema: FUSION_SCHEMA,
        key: "glass-glare",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Display,
        title: "Shadow",
        sub: "Soft shadow under glass",
        schema: FUSION_SCHEMA,
        key: "glass-shadow",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Pointer,
        title: "Light follows the pointer",
        sub: "Soft light spot",
        schema: FUSION_SCHEMA,
        key: "glass-light-follows",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Personalise,
        title: "Accent gradient",
        sub: "Shade panels with the accent",
        schema: FUSION_SCHEMA,
        key: "accent-gradient",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Personalise,
        title: "Gradient shape",
        sub: "Direction of the gradient",
        schema: FUSION_SCHEMA,
        key: "accent-gradient-style",
        kind: Kind::Choice(&[
            ("CORNER", "Corner"),
            ("TOP", "Top"),
            ("SIDE", "Side"),
            ("CENTER", "Centre"),
            ("AURORA", "Aurora"),
            ("SUNSET", "Sunset"),
            ("OCEAN", "Ocean"),
            ("BERRY", "Berry"),
        ]),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Personalise,
        title: "Gradient strength",
        sub: "Percent",
        schema: FUSION_SCHEMA,
        key: "accent-gradient-strength",
        kind: Kind::Int(10, 100, 5),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Personalise,
        title: "Icons in the accent colour",
        sub: "Start and apps buttons",
        schema: FUSION_SCHEMA,
        key: "icons-follow-accent",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Image,
        title: "Blur the wallpaper",
        sub: "While windows are open",
        schema: FUSION_SCHEMA,
        key: "wallpaper-blur",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Image,
        title: "Wallpaper blur strength",
        sub: "Percent",
        schema: FUSION_SCHEMA,
        key: "wallpaper-blur-strength",
        kind: Kind::Int(5, 100, 5),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Display,
        title: "Blurred overview",
        sub: "Behind the overview and app grid",
        schema: FUSION_SCHEMA,
        key: "overview-blur",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Display,
        title: "Overview blur strength",
        sub: "Percent",
        schema: FUSION_SCHEMA,
        key: "overview-blur-strength",
        kind: Kind::Int(10, 100, 5),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Display,
        title: "See-through windows",
        sub: "Blur behind chosen apps' windows",
        schema: FUSION_SCHEMA,
        key: "window-blur",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Display,
        title: "Window opacity",
        sub: "Blurred windows, percent",
        schema: FUSION_SCHEMA,
        key: "window-blur-opacity",
        kind: Kind::Int(50, 100, 5),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Clock,
        title: "Animation speed",
        sub: "Every shell animation",
        schema: FUSION_SCHEMA,
        key: "tweak-animation-speed",
        kind: Kind::Choice(&[("FAST", "Fast"), ("NORMAL", "Normal"), ("SLOW", "Slow")]),
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Apps,
        title: "Workspace popup",
        sub: "When switching workspaces",
        schema: FUSION_SCHEMA,
        key: "tweak-workspace-popup",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Bell,
        title: "Raise apps that need attention",
        sub: "Instead of a notification",
        schema: FUSION_SCHEMA,
        key: "tweak-focus-attention",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Glass & effects",
        icon: Icon::Apps,
        title: "Hide the overview dash",
        sub: "The taskbar replaces it",
        schema: FUSION_SCHEMA,
        key: "hide-overview-dash",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Storage,
        title: "Desktop organizer",
        sub: "Files of the Desktop folder on the desktop",
        schema: FUSION_SCHEMA,
        key: "deepin-organizer",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Apps,
        title: "Organizer style",
        sub: "Sorted boxes or plain icons",
        schema: FUSION_SCHEMA,
        key: "deepin-organizer-style",
        kind: Kind::Choice(&[("COLLECTIONS", "Collections"), ("ICONS", "Icons")]),
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Apps,
        title: "Collection size",
        sub: "Default size of a collection",
        schema: FUSION_SCHEMA,
        key: "deepin-organizer-size",
        kind: Kind::Choice(&[("SMALL", "Small"), ("MEDIUM", "Medium"), ("LARGE", "Large")]),
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Home,
        title: "Add to Desktop",
        sub: "In app menus",
        schema: FUSION_SCHEMA,
        key: "add-to-desktop",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Display,
        title: "Picture-in-picture on top",
        sub: "Keep PiP windows above others",
        schema: FUSION_SCHEMA,
        key: "pip-on-top",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Display,
        title: "PiP on every workspace",
        sub: "Show PiP windows everywhere",
        schema: FUSION_SCHEMA,
        key: "pip-all-workspaces",
        kind: Kind::Bool,
    },
    Item {
        section: "Fusion Desk · Desktop",
        icon: Icon::Battery,
        title: "Laptop switches",
        sub: "In the control centre",
        schema: FUSION_SCHEMA,
        key: "laptop-controls",
        kind: Kind::Bool,
    },
];

#[derive(Clone)]
pub struct Autostart {
    pub file: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct Extension {
    pub uuid: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub state: i64,
    pub user: bool,
    pub has_prefs: bool,
    pub version: String,
    pub path: String,
    pub schema: String,
    /// Version on disk when it differs from the running one (update waits for a new login).
    pub pending: String,
}

#[derive(Clone)]
pub struct Opt {
    pub key: String,
    pub label: String,
    pub on: bool,
}

#[derive(Clone)]
pub struct Found {
    pub uuid: String,
    pub name: String,
    pub description: String,
}

struct Loaded {
    values: HashMap<String, String>,
    fonts: Vec<String>,
    themes: HashMap<u8, Vec<String>>,
    autostart: Vec<Autostart>,
    launchers: Vec<(String, String)>,
}

enum Done {
    Message(Result<String, String>),
    Extensions(Vec<Extension>),
    Found(Result<Vec<Found>, String>),
    Options(String, Vec<Opt>),
    FusionInstalled(Result<String, String>),
}

#[derive(Default)]
pub struct State {
    loaded: Option<Loaded>,
    rx: Option<Receiver<Loaded>>,
    jobs: Vec<Receiver<Done>>,
    extensions: Option<Vec<Extension>>,
    ext_loading: bool,
    user_extensions_on: Option<bool>,
    query: String,
    found: Vec<Found>,
    searching: bool,
    open: std::collections::HashSet<String>,
    options: HashMap<String, Vec<Opt>>,
    fusion_installing: bool,
}

/// Extension schemas are not in the system schema path: point gsettings at
/// the extension's own `schemas` folder.
/// Whether Fusion Desk's settings schema is installed.
pub fn fusion_installed() -> bool {
    !schema_args(FUSION_SCHEMA).is_empty()
}

/// Fusion Desk's glass settings, or `None` when it is not installed.
pub fn fusion_glass() -> Option<crate::theme::Glass> {
    if !fusion_installed() {
        return None;
    }
    let values = gsettings_dump(FUSION_SCHEMA);
    if values.is_empty() {
        return None;
    }
    let mut glass = crate::theme::Glass::default();
    for (key, value) in &values {
        if key.starts_with("glass-") {
            glass.set(key, &plain(value));
        }
    }
    Some(glass)
}

/// Write every glass setting to Fusion Desk.
pub fn set_fusion_glass(glass: &crate::theme::Glass) -> bool {
    let extra = schema_args(FUSION_SCHEMA);
    if extra.is_empty() {
        return false;
    }
    crate::theme::Glass::KEYS.iter().all(|key| {
        Command::new("gsettings")
            .args(&extra)
            .args(["set", FUSION_SCHEMA, key, &glass.get(key)])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

fn schema_args(schema: &str) -> Vec<String> {
    if schema != FUSION_SCHEMA {
        return Vec::new();
    }
    let candidates = [
        format!(
            "{}/.local/share/gnome-shell/extensions/{FUSION_UUID}/schemas",
            home()
        ),
        format!("/usr/share/gnome-shell/extensions/{FUSION_UUID}/schemas"),
        format!("/usr/local/share/gnome-shell/extensions/{FUSION_UUID}/schemas"),
    ];
    candidates
        .into_iter()
        .find(|d| std::path::Path::new(d).join("gschemas.compiled").exists())
        .map(|d| vec!["--schemadir".to_owned(), d])
        .unwrap_or_default()
}

fn gsettings_dump(schema: &str) -> Vec<(String, String)> {
    let extra = schema_args(schema);
    if schema == FUSION_SCHEMA && extra.is_empty() {
        return Vec::new();
    }
    let out = Command::new("gsettings")
        .args(&extra)
        .args(["list-recursively", schema])
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut it = line.splitn(3, ' ');
            let (_, key, value) = (it.next()?, it.next()?, it.next()?);
            Some((key.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// GVariant text to a plain string: 'Bottom' -> Bottom, uint32 48 -> 48.
fn plain(value: &str) -> String {
    let v = value.trim();
    let v = v
        .strip_prefix("uint32 ")
        .or_else(|| v.strip_prefix("int32 "))
        .unwrap_or(v);
    v.trim_matches('\'').to_owned()
}

fn dirs(paths: &[String], must_have: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for base in paths {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.join(must_have).exists() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !names.iter().any(|n| n == name) {
                    names.push(name.to_owned());
                }
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn data_dirs() -> Vec<String> {
    let mut list = vec![std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.local/share", home()))];
    list.extend(
        std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned())
            .split(':')
            .map(str::to_owned),
    );
    list
}

fn autostart_dir() -> std::path::PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{}/.config", home()));
    std::path::PathBuf::from(config).join("autostart")
}

fn desktop_field(text: &str, field: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some(v) = line.strip_prefix(field).and_then(|r| r.strip_prefix('=')) {
                return Some(v.trim().to_owned());
            }
        }
    }
    None
}

fn read_autostart() -> Vec<Autostart> {
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(autostart_dir()) else {
        return list;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = desktop_field(&text, "Name").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned()
        });
        let enabled = desktop_field(&text, "Hidden").as_deref() != Some("true")
            && desktop_field(&text, "X-GNOME-Autostart-enabled").as_deref() != Some("false");
        list.push(Autostart {
            file: path.to_string_lossy().into_owned(),
            name,
            enabled,
        });
    }
    list.sort_by_key(|a| a.name.to_lowercase());
    list
}

fn read_launchers() -> Vec<(String, String)> {
    let mut seen: HashMap<String, String> = HashMap::new();
    for base in data_dirs() {
        let Ok(entries) = std::fs::read_dir(format!("{base}/applications")) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if !id.ends_with(".desktop") || seen.contains_key(&id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if desktop_field(&text, "NoDisplay").as_deref() == Some("true")
                || desktop_field(&text, "Type").as_deref() != Some("Application")
            {
                continue;
            }
            if let Some(name) = desktop_field(&text, "Name") {
                seen.insert(id, path.to_string_lossy().into_owned() + "\u{1}" + &name);
            }
        }
    }
    let mut list: Vec<(String, String)> = seen
        .into_values()
        .filter_map(|v| {
            v.split_once('\u{1}')
                .map(|(p, n)| (p.to_owned(), n.to_owned()))
        })
        .collect();
    list.sort_by_key(|(_, n)| n.to_lowercase());
    list
}

fn load() -> Loaded {
    let mut values = HashMap::new();
    let mut schemas: Vec<&str> = ITEMS.iter().map(|i| i.schema).collect();
    schemas.sort();
    schemas.dedup();
    for schema in schemas {
        for (key, value) in gsettings_dump(schema) {
            values.insert(format!("{schema} {key}"), value);
        }
    }
    let fonts = Command::new("fc-list")
        .args([":", "family"])
        .output()
        .map(|o| {
            let mut list: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split(',').next())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty() && !s.starts_with('.'))
                .collect();
            list.sort_by_key(|s| s.to_lowercase());
            list.dedup();
            list
        })
        .unwrap_or_default();
    let data = data_dirs();
    let mut themes = HashMap::new();
    let theme_dirs: Vec<String> = data
        .iter()
        .map(|d| format!("{d}/themes"))
        .chain(std::iter::once(format!("{}/.themes", home())))
        .collect();
    let icon_dirs: Vec<String> = data
        .iter()
        .map(|d| format!("{d}/icons"))
        .chain(std::iter::once(format!("{}/.icons", home())))
        .collect();
    themes.insert(ThemeKind::Gtk as u8, dirs(&theme_dirs, "gtk-3.0"));
    themes.insert(ThemeKind::Icons as u8, dirs(&icon_dirs, "index.theme"));
    themes.insert(ThemeKind::Cursor as u8, dirs(&icon_dirs, "cursors"));
    Loaded {
        values,
        fonts,
        themes,
        autostart: read_autostart(),
        launchers: read_launchers(),
    }
}

/// Write a value the way gsettings expects for this kind.
fn write(item: &Item, value: &str) -> bool {
    let arg = match item.kind {
        Kind::Bool | Kind::BoolInverted | Kind::Int(..) => value.to_owned(),
        _ => format!("'{}'", value.replace('\'', "\\'")),
    };
    Command::new("gsettings")
        .args(schema_args(item.schema))
        .args(["set", item.schema, item.key, &arg])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn section_title(ui: &mut Ui, app: &App, text: &str) {
    let pal = app.pal();
    ui.add_space(18.0);
    widgets::section_label(ui, &pal, text);
}

fn show(app: &mut App, ui: &mut Ui, only: Option<usize>) {
    let pal = app.pal();
    match only {
        Some(i) => widgets::h1(ui, &pal, section_name(i as u8)),
        None => {
            widgets::h1(ui, &pal, "Desktop & windows");
            ui.label(
                RichText::new("Multitasking, dock, windows, fonts and themes — the Ubuntu and Tweaks options in one place.")
                    .font(regular(12.5))
                    .color(pal.text_faint),
            );
        }
    }

    if let Some(rx) = &app.desk.rx {
        if let Ok(loaded) = rx.try_recv() {
            app.desk.loaded = Some(loaded);
            app.desk.rx = None;
        }
    }
    if app.desk.loaded.is_none() {
        if app.desk.rx.is_none() {
            let (tx, rx) = mpsc::channel();
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || {
                let _ = tx.send(load());
                ctx.request_repaint();
            });
            app.desk.rx = Some(rx);
        }
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Reading your desktop settings…").color(pal.text_muted));
        });
        return;
    }

    // Which sections to draw, and the heading over each (None = the page
    // title already names it).
    let mut plan: Vec<(&'static str, Option<&'static str>)> = Vec::new();
    match only {
        None => {
            plan.extend(SECTIONS.iter().map(|s| (*s, Some(*s))));
            plan.extend(FUSION_GROUPS.iter().map(|g| (*g, Some(*g))));
        }
        Some(i) if i < SECTIONS.len() => plan.push((SECTIONS[i], None)),
        // One Fusion Desk group on its own page.
        Some(i) if i > FUSION_INDEX && i <= FUSION_INDEX + FUSION_GROUPS.len() => {
            plan.push((FUSION_GROUPS[i - FUSION_INDEX - 1], None))
        }
        Some(FUSION_INDEX) => plan.extend(
            FUSION_GROUPS
                .iter()
                .map(|g| (*g, Some(g.trim_start_matches("Fusion Desk · ")))),
        ),
        Some(_) => {}
    }
    let mut set: Option<(usize, String)> = None;
    let mut fusion_shown = false;
    // Group pages skip the status card unless Fusion Desk is missing (then
    // it holds the Install button).
    let fusion_missing = !app
        .desk
        .loaded
        .as_ref()
        .is_some_and(|l| l.values.keys().any(|k| k.starts_with(FUSION_SCHEMA)));
    let fusion_card = only.is_none() || only == Some(FUSION_INDEX) || fusion_missing;
    for (section, heading) in plan {
        if section.starts_with("Fusion Desk") && !fusion_shown && fusion_card {
            fusion_shown = true;
            if only.is_none() {
                section_title(ui, app, "Fusion Desk");
            } else {
                ui.add_space(14.0);
            }
            fusion_header(app, ui);
        }
        let rows: Vec<usize> = ITEMS
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                it.section == section
                    && app.desk.loaded.as_ref().is_some_and(|l| {
                        l.values.contains_key(&format!("{} {}", it.schema, it.key))
                    })
            })
            .map(|(i, _)| i)
            .collect();
        if rows.is_empty() {
            continue;
        }
        match heading {
            Some(text) => section_title(ui, app, text),
            None => ui.add_space(14.0),
        }
        let loaded = app.desk.loaded.as_ref().unwrap();
        widgets::card(ui, &pal, |ui| {
            for (n, &i) in rows.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let it = &ITEMS[i];
                let id = format!("{} {}", it.schema, it.key);
                let raw = loaded.values.get(&id).cloned().unwrap_or_default();
                let current = plain(&raw);
                match it.kind {
                    Kind::Bool | Kind::BoolInverted => {
                        let invert = matches!(it.kind, Kind::BoolInverted);
                        let mut on = (current == "true") != invert;
                        if widgets::row(
                            ui,
                            &pal,
                            pal.accent,
                            it.icon,
                            it.title,
                            it.sub,
                            Right::Switch(&mut on),
                        )
                        .toggled()
                        {
                            set = Some((i, ((on != invert) as u8 == 1).to_string()));
                        }
                    }
                    Kind::Choice(options) => {
                        let opts: Vec<(String, String)> = options
                            .iter()
                            .map(|(v, l)| ((*v).to_owned(), (*l).to_owned()))
                            .collect();
                        if let Some(v) = combo_row(
                            ui, &pal, it.icon, it.title, it.sub, &id, &opts, &current, 230.0,
                        ) {
                            set = Some((i, v));
                        }
                    }
                    Kind::Int(min, max, step) => {
                        let mut value = current.parse::<f32>().unwrap_or(min as f32);
                        let sub = format!("{} · {}", it.sub, value as i64);
                        if range_row(
                            ui,
                            &pal,
                            it.icon,
                            it.title,
                            sub,
                            &mut value,
                            min as f32,
                            max as f32,
                            step as f32,
                        ) {
                            set = Some((i, (value as i64).to_string()));
                        }
                    }
                    Kind::Font => {
                        let (family, size) = match current.rsplit_once(' ') {
                            Some((f, s)) if s.parse::<f32>().is_ok() => {
                                (f.to_owned(), s.to_owned())
                            }
                            _ => (current.clone(), "11".to_owned()),
                        };
                        let opts: Vec<(String, String)> = loaded
                            .fonts
                            .iter()
                            .map(|f| (f.clone(), f.clone()))
                            .collect();
                        let sub = format!("{} · {} pt", it.sub, size);
                        if let Some(f) = combo_row(
                            ui, &pal, it.icon, it.title, &sub, &id, &opts, &family, 230.0,
                        ) {
                            set = Some((i, format!("{f} {size}")));
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(ROW_PAD + 42.0);
                            ui.label(
                                RichText::new("Size")
                                    .font(regular(12.0))
                                    .color(pal.text_muted),
                            );
                            let mut s = size.parse::<f32>().unwrap_or(11.0);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut s)
                                        .range(6.0..=36.0)
                                        .speed(0.2)
                                        .fixed_decimals(0),
                                )
                                .changed()
                            {
                                set = Some((i, format!("{family} {}", s.round())));
                            }
                        });
                        ui.add_space(8.0);
                    }
                    Kind::Theme(kind) => {
                        let opts: Vec<(String, String)> = loaded
                            .themes
                            .get(&(kind as u8))
                            .map(|l| l.iter().map(|t| (t.clone(), t.clone())).collect())
                            .unwrap_or_default();
                        if let Some(v) = combo_row(
                            ui, &pal, it.icon, it.title, it.sub, &id, &opts, &current, 230.0,
                        ) {
                            set = Some((i, v));
                        }
                    }
                }
            }
        });
    }
    if let Some((i, value)) = set {
        let it = &ITEMS[i];
        if write(it, &value) {
            let stored = match it.kind {
                Kind::Bool | Kind::BoolInverted | Kind::Int(..) => value,
                _ => format!("'{value}'"),
            };
            if let Some(l) = app.desk.loaded.as_mut() {
                l.values.insert(format!("{} {}", it.schema, it.key), stored);
            }
            app.notify(format!("{} updated", it.title));
        } else {
            app.notify_error(format!("Could not change {}", it.title.to_lowercase()));
        }
    }

    if only.is_none() {
        startup_apps(app, ui, true);
    } else if only == Some(SECTIONS.len()) {
        ui.add_space(14.0);
        startup_apps(app, ui, false);
    }
}

/// Fusion Desk status: install it when missing, turn it on or off, open its
/// own settings window.
fn fusion_header(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let ctx = ui.ctx().clone();
    poll(app, &ctx);
    if app.desk.extensions.is_none() && !app.desk.ext_loading {
        refresh_extensions(app, &ctx);
    }
    let ext = app
        .desk
        .extensions
        .as_ref()
        .and_then(|list| list.iter().find(|e| e.uuid == FUSION_UUID).cloned());
    let installing = app.desk.fusion_installing;
    let mut verb: Option<&'static str> = None;
    let mut install = false;
    widgets::card(ui, &pal, |ui| match &ext {
        None if app.desk.extensions.is_none() => {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD);
                ui.spinner();
                ui.label(RichText::new("Looking for Fusion Desk…").color(pal.text_muted));
            });
            ui.add_space(14.0);
        }
        None => {
            let sub = if installing {
                "Downloading and installing the latest release…"
            } else {
                "Taskbar, start menu, widgets and glass in one GNOME extension. Not installed."
            };
            widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Sparkle,
                "Fusion Desk",
                sub,
                Right::None,
            );
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD + 42.0);
                if installing {
                    ui.spinner();
                } else if widgets::button(
                    ui,
                    &pal,
                    pal.accent,
                    "Install Fusion Desk",
                    Some(Icon::Hub),
                )
                .clicked()
                {
                    install = true;
                }
            });
            ui.add_space(12.0);
        }
        Some(e) => {
            let mut on = e.enabled;
            let sub = format!(
                "Version {} · {}{}",
                e.version,
                state_text(e),
                if e.pending.is_empty() {
                    ""
                } else {
                    " · update applies after you sign in again"
                }
            );
            if widgets::row(
                ui,
                &pal,
                pal.accent,
                Icon::Sparkle,
                "Fusion Desk",
                &sub,
                Right::Switch(&mut on),
            )
            .toggled()
            {
                verb = Some(if on { "enable" } else { "disable" });
            }
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD + 42.0);
                if widgets::ghost_button(ui, &pal, "All Fusion Desk settings", None).clicked() {
                    verb = Some("prefs");
                }
            });
            ui.add_space(12.0);
        }
    });
    if let Some(verb) = verb {
        ext_action(app, &ctx, verb, FUSION_UUID.to_owned());
    }
    if install {
        app.desk.fusion_installing = true;
        spawn(app, &ctx, || Done::FusionInstalled(install_fusion()));
    }
}

/// Run `gnome-extensions <verb> <uuid>` off the UI thread.
fn ext_action(app: &mut App, ctx: &Context, verb: &'static str, uuid: String) {
    spawn(app, &ctx, move || {
        let mut res = run("gnome-extensions", &[verb, &uuid]);
        let mut note = String::new();
        // GNOME shows one extension settings window at a time; a hidden
        // or stuck one blocks every later open until its service restarts.
        if verb == "prefs"
            && res.as_ref().is_err_and(|e| e.contains("Already showing"))
            && restart_prefs_service()
        {
            res = run("gnome-extensions", &[verb, &uuid]);
            note = "Closed the other extension settings window".to_owned();
        }
        Done::Message(match verb {
            "prefs" => res
                .map(|_| note)
                .map_err(|e| format!("Could not open settings: {e}")),
            _ => res.map(|_| {
                format!(
                    "Extension turned {}",
                    if verb == "enable" { "on" } else { "off" }
                )
            }),
        })
    });
}

/// Download the newest Fusion Desk release zip and install it for this user.
fn install_fusion() -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("win11-settings-fusion-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip = dir.join(format!("{FUSION_UUID}.shell-extension.zip"));
    let url = format!(
        "https://github.com/{FUSION_REPO}/releases/latest/download/{FUSION_UUID}.shell-extension.zip"
    );
    let zip_path = zip.to_string_lossy().into_owned();
    run(
        "curl",
        &["-fsSL", "--max-time", "300", "-o", &zip_path, &url],
    )
    .map_err(|e| format!("Download failed: {e}"))?;
    let res = run("gnome-extensions", &["install", "--force", &zip_path]);
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_dir(&dir);
    res.map_err(|e| format!("Install failed: {e}"))?;
    // The running shell only loads new extensions after a new login on
    // Wayland; enabling now makes it start then.
    let _ = run("gnome-extensions", &["enable", FUSION_UUID]);
    Ok("Fusion Desk installed. Sign out and back in to start it.".to_owned())
}

fn startup_apps(app: &mut App, ui: &mut Ui, heading: bool) {
    let pal = app.pal();
    if heading {
        section_title(ui, app, "Startup apps");
    }
    let Some(loaded) = app.desk.loaded.as_ref() else {
        return;
    };
    let mut toggle: Option<(usize, bool)> = None;
    let mut remove: Option<usize> = None;
    let mut add: Option<String> = None;
    widgets::card(ui, &pal, |ui| {
        if loaded.autostart.is_empty() {
            widgets::empty_note(ui, &pal, "No apps start when you sign in.");
        }
        for (i, entry) in loaded.autostart.iter().enumerate() {
            if i > 0 {
                widgets::divider(ui, &pal);
            }
            let mut on = entry.enabled;
            ui.horizontal(|ui| {
                let row = ui.scope(|ui| {
                    ui.set_width(ui.available_width() - 90.0);
                    widgets::row(
                        ui,
                        &pal,
                        pal.accent,
                        Icon::Power,
                        &entry.name,
                        "Starts when you sign in",
                        Right::Switch(&mut on),
                    )
                });
                if row.inner.toggled() {
                    toggle = Some((i, on));
                }
                if widgets::ghost_button(ui, &pal, "Remove", None).clicked() {
                    remove = Some(i);
                }
            });
        }
        widgets::divider(ui, &pal);
        let opts: Vec<(String, String)> = loaded.launchers.clone();
        if let Some(path) = combo_row(
            ui,
            &pal,
            Icon::Apps,
            "Add a startup app",
            "Start it every time you sign in",
            "autostart-add",
            &opts,
            "",
            230.0,
        ) {
            add = Some(path);
        }
    });
    let dir = autostart_dir();
    let mut changed = false;
    if let Some((i, on)) = toggle {
        let file = loaded.autostart[i].file.clone();
        if let Ok(text) = std::fs::read_to_string(&file) {
            let mut out: Vec<String> = text
                .lines()
                .filter(|l| {
                    !l.starts_with("X-GNOME-Autostart-enabled=") && !l.starts_with("Hidden=")
                })
                .map(str::to_owned)
                .collect();
            let at = out
                .iter()
                .position(|l| l.trim() == "[Desktop Entry]")
                .map(|p| p + 1)
                .unwrap_or(out.len());
            out.insert(at, format!("X-GNOME-Autostart-enabled={on}"));
            changed = std::fs::write(&file, out.join("\n") + "\n").is_ok();
        }
    }
    if let Some(i) = remove {
        changed = std::fs::remove_file(&loaded.autostart[i].file).is_ok();
    }
    if let Some(path) = add {
        let _ = std::fs::create_dir_all(&dir);
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_owned());
        if let Some(name) = name {
            changed = std::fs::copy(&path, dir.join(name)).is_ok();
        }
    }
    if changed {
        if let Some(l) = app.desk.loaded.as_mut() {
            l.autostart = read_autostart();
        }
        app.notify("Startup apps updated");
    }
}

// ---------------------------------------------------------------- Extensions

const LIST_PY: &str = r#"
import json
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
r = bus.call_sync('org.gnome.Shell.Extensions', '/org/gnome/Shell/Extensions',
    'org.gnome.Shell.Extensions', 'ListExtensions', None, None, 0, 5000, None).unpack()[0]
def ver(d):
    v = d.get('version-name', d.get('version', ''))
    if isinstance(v, float) and v.is_integer():
        v = int(v)
    return str(v)
def disk(path):
    try:
        return ver(json.load(open(path + '/metadata.json')))
    except Exception:
        return ''
s = Gio.Settings.new('org.gnome.shell')
on = set(s.get_strv('enabled-extensions'))
out = [dict(uuid=u, name=d.get('name', u), description=d.get('description', ''),
    enabled=u in on, state=int(d.get('state', 0)), user=int(d.get('type', 2)) == 2,
    has_prefs=bool(d.get('hasPrefs', False)), version=ver(d),
    path=d.get('path', ''), schema=d.get('settings-schema', ''), disk=disk(d.get('path', '')))
    for u, d in r.items()]
print(json.dumps(dict(user_on=not s.get_boolean('disable-user-extensions'), list=out)))
"#;

/// argv: schema dir ('' = installed system-wide), schema id [, key, true|false]
const OPTS_PY: &str = r#"
import json, os, sys
from gi.repository import Gio
d, sid = sys.argv[1], sys.argv[2]
default = Gio.SettingsSchemaSource.get_default()
src = Gio.SettingsSchemaSource.new_from_directory(d, default, False) if d and os.path.isdir(d) else default
sch = src.lookup(sid, True)
s = Gio.Settings.new_full(sch, None, None)
if len(sys.argv) > 4:
    s.set_boolean(sys.argv[3], sys.argv[4] == 'true')
    Gio.Settings.sync()
print(json.dumps([dict(key=k, label=(sch.get_key(k).get_summary() or k), on=s.get_boolean(k))
    for k in sorted(sch.list_keys()) if sch.get_key(k).get_value_type().dup_string() == 'b']))
"#;

/// Fusion Desk features as simple switches (the rest stays in its own settings).
const FUSION_KEYS: &[(&str, &str)] = &[
    ("menu-visible", "Start button"),
    ("activities-visible", "Activities button"),
    ("taskbar-visible", "Taskbar"),
    ("windows-extras-in-fusion", "Search and Task View buttons"),
    ("settings-button", "Settings on the taskbar"),
    ("apps-visible", "Show Applications button"),
    ("title-visible", "Window title"),
    ("workspaces-visible", "Workspace switcher"),
    ("media-visible", "Media controls"),
    ("sysmon-visible", "System monitor"),
    ("clipboard-visible", "Clipboard history icon"),
    ("clipboard-super-v", "Super+V opens the clipboard"),
    ("caffeine-visible", "Caffeine (keep awake)"),
    ("screenshot-visible", "Screenshot button"),
    ("apphub-visible", "App Hub button"),
    ("overflow-visible", "Hidden tray icons arrow"),
    ("desktop-visible", "Show Desktop strip"),
    ("battery-standalone", "Battery next to the system menu"),
    ("icons-follow-accent", "Icons in the accent colour"),
    ("deepin-control-button", "Control centre"),
    ("deepin-organizer", "Desktop organizer"),
    ("deepin-floating-taskbar", "Floating taskbar"),
    ("wallpaper-blur", "Blur the wallpaper behind windows"),
    ("overview-blur", "Blurred overview"),
    ("window-blur", "See-through blurred windows"),
    ("add-to-desktop", "Add to Desktop in app menus"),
    ("charge-limit", "Battery charge limit"),
    (
        "avoid-duplicates",
        "Hide items other extensions already show",
    ),
];

/// A compact on/off line used inside an extension's options list.
fn option_row(ui: &mut egui::Ui, pal: &crate::theme::Palette, label: &str, on: &mut bool) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(egui::vec2(4.0, 1.0)), pal.corner(4), pal.hover);
    }
    let left = rect.left() + ROW_PAD + 54.0;
    let sw = egui::Rect::from_center_size(
        egui::pos2(rect.right() - ROW_PAD - 20.0, rect.center().y),
        egui::vec2(40.0, 20.0),
    );
    let max_w = sw.left() - 16.0 - left;
    let mut text = label.to_owned();
    let font = regular(13.0);
    while text.chars().count() > 8
        && ui
            .painter()
            .layout_no_wrap(text.clone(), font.clone(), pal.text)
            .size()
            .x
            > max_w
    {
        text = text
            .chars()
            .take(text.chars().count() - 2)
            .collect::<String>()
            .trim_end()
            .to_owned()
            + "…";
        text = text.replace("……", "…");
    }
    ui.painter().text(
        egui::pos2(left, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        pal.text,
    );
    let t = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("sw"), *on, 0.11);
    widgets::paint_switch(ui.painter(), sw, t, pal, pal.accent);
    let clicked = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
        .clicked();
    if clicked {
        *on = !*on;
    }
    clicked
}

fn load_options(app: &mut App, ctx: &Context, e: &Extension, set: Option<(String, bool)>) {
    let dir = if e.path.is_empty() {
        String::new()
    } else {
        format!("{}/schemas", e.path)
    };
    let (uuid, schema) = (e.uuid.clone(), e.schema.clone());
    spawn(app, ctx, move || {
        let mut args = vec!["-c".to_owned(), OPTS_PY.to_owned(), dir, schema];
        if let Some((k, v)) = set {
            args.push(k);
            args.push(v.to_string());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut opts: Vec<Opt> = run("python3", &refs)
            .ok()
            .and_then(|out| serde_json::from_str::<Vec<serde_json::Value>>(&out).ok())
            .unwrap_or_default()
            .iter()
            .map(|v| Opt {
                key: v["key"].as_str().unwrap_or("").to_owned(),
                label: v["label"].as_str().unwrap_or("").to_owned(),
                on: v["on"].as_bool().unwrap_or(false),
            })
            .collect();
        if uuid.starts_with("fusion-desk@") {
            opts = FUSION_KEYS
                .iter()
                .filter_map(|(k, label)| {
                    opts.iter().find(|o| o.key == *k).map(|o| Opt {
                        label: (*label).to_owned(),
                        ..o.clone()
                    })
                })
                .collect();
        }
        Done::Options(uuid, opts)
    });
}

const REMOTE_PY: &str = r#"
import sys
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
r = bus.call_sync('org.gnome.Shell.Extensions', '/org/gnome/Shell/Extensions',
    'org.gnome.Shell.Extensions', 'InstallRemoteExtension', GLib.Variant('(s)', (sys.argv[1],)),
    None, 0, 600000, None).unpack()[0]
print(r)
"#;

const SEARCH_PY: &str = r#"
import json, sys, urllib.parse, urllib.request
q = urllib.parse.urlencode({'search': sys.argv[1], 'shell_version': sys.argv[2], 'n_per_page': 20})
req = urllib.request.Request('https://extensions.gnome.org/extension-query/?' + q, headers={'User-Agent': 'Settings'})
d = json.load(urllib.request.urlopen(req, timeout=15))
print(json.dumps([dict(uuid=e['uuid'], name=e['name'], description=e['description'].split('\n')[0][:160])
    for e in d.get('extensions', [])]))
"#;

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    match Command::new(cmd).args(args).output() {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_owned()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr)
            .lines()
            .last()
            .unwrap_or("failed")
            .to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

/// Stop the org.gnome.Shell.Extensions service (D-Bus starts it again on the next call).
fn restart_prefs_service() -> bool {
    const NAME: &str = "org.gnome.Shell.Extensions";
    let dbus = |method: &str| {
        run(
            "gdbus",
            &[
                "call",
                "--session",
                "-d",
                "org.freedesktop.DBus",
                "-o",
                "/org/freedesktop/DBus",
                "-m",
                &format!("org.freedesktop.DBus.{method}"),
                NAME,
            ],
        )
    };
    let Ok(out) = dbus("GetConnectionUnixProcessID") else {
        return false;
    };
    // gdbus prints "(uint32 1234,)".
    let pid: String = out
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    if pid.is_empty() || !String::from_utf8_lossy(&cmdline).contains(NAME) {
        return false;
    }
    if run("kill", &[&pid]).is_err() {
        return false;
    }
    for _ in 0..30 {
        if dbus("NameHasOwner").is_ok_and(|o| o.contains("false")) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

fn spawn(app: &mut App, ctx: &Context, job: impl FnOnce() -> Done + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(job());
        ctx.request_repaint();
    });
    app.desk.jobs.push(rx);
}

fn refresh_extensions(app: &mut App, ctx: &Context) {
    app.desk.ext_loading = true;
    spawn(app, ctx, || {
        let list = run("python3", &["-c", LIST_PY])
            .ok()
            .and_then(|out| serde_json::from_str::<serde_json::Value>(&out).ok());
        let Some(v) = list else {
            return Done::Extensions(Vec::new());
        };
        let mut exts: Vec<Extension> = v["list"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| Extension {
                        uuid: e["uuid"].as_str().unwrap_or("").to_owned(),
                        name: e["name"].as_str().unwrap_or("").to_owned(),
                        description: e["description"]
                            .as_str()
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("")
                            .to_owned(),
                        enabled: e["enabled"].as_bool().unwrap_or(false),
                        state: e["state"].as_i64().unwrap_or(0),
                        user: e["user"].as_bool().unwrap_or(true),
                        has_prefs: e["has_prefs"].as_bool().unwrap_or(false),
                        version: e["version"].as_str().unwrap_or("").to_owned(),
                        path: e["path"].as_str().unwrap_or("").to_owned(),
                        schema: e["schema"].as_str().unwrap_or("").to_owned(),
                        pending: {
                            let (run, disk) = (
                                e["version"].as_str().unwrap_or(""),
                                e["disk"].as_str().unwrap_or(""),
                            );
                            if disk.is_empty() || disk == run {
                                String::new()
                            } else {
                                disk.to_owned()
                            }
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        exts.sort_by_key(|e| (!e.user, e.name.to_lowercase()));
        // The master switch rides along as a fake first entry.
        let user_on = v["user_on"].as_bool().unwrap_or(true);
        exts.insert(
            0,
            Extension {
                uuid: String::new(),
                name: String::new(),
                description: String::new(),
                enabled: user_on,
                state: 0,
                user: true,
                has_prefs: false,
                version: String::new(),
                path: String::new(),
                schema: String::new(),
                pending: String::new(),
            },
        );
        Done::Extensions(exts)
    });
}

fn poll(app: &mut App, ctx: &Context) {
    let mut done = Vec::new();
    app.desk.jobs.retain(|rx| match rx.try_recv() {
        Ok(d) => {
            done.push(d);
            false
        }
        Err(mpsc::TryRecvError::Empty) => true,
        Err(_) => false,
    });
    for d in done {
        match d {
            Done::Extensions(mut list) => {
                app.desk.ext_loading = false;
                if !list.is_empty() {
                    let master = list.remove(0);
                    app.desk.user_extensions_on = Some(master.enabled);
                }
                // Screenshot hook: open one extension's options straight away.
                if let Ok(want) = std::env::var("WIN11_SETTINGS_EXT_OPEN") {
                    if let Some(e) = list
                        .iter()
                        .find(|e| e.uuid == want && !e.schema.is_empty())
                        .cloned()
                    {
                        if app.desk.open.insert(e.uuid.clone()) {
                            load_options(app, ctx, &e, None);
                        }
                    }
                }
                app.desk.extensions = Some(list);
            }
            Done::Message(Ok(msg)) => {
                if msg.is_empty() {
                    continue;
                }
                app.notify(msg);
                refresh_extensions(app, ctx);
            }
            Done::Message(Err(err)) => app.notify_error(err),
            Done::FusionInstalled(res) => {
                app.desk.fusion_installing = false;
                match res {
                    Ok(msg) => app.notify(msg),
                    Err(err) => app.notify_error(err),
                }
                // Re-read: the Fusion rows appear once its schema is on disk.
                app.desk.loaded = None;
                refresh_extensions(app, ctx);
            }
            Done::Options(uuid, opts) => {
                app.desk.options.insert(uuid, opts);
            }
            Done::Found(res) => {
                app.desk.searching = false;
                match res {
                    Ok(list) => {
                        if list.is_empty() {
                            app.notify("No extensions found");
                        }
                        app.desk.found = list;
                    }
                    Err(err) => app.notify_error(format!("Search failed: {err}")),
                }
            }
        }
    }
}

fn state_text(e: &Extension) -> &'static str {
    match e.state {
        1 => "Running",
        3 => "Error",
        4 => "Not made for this GNOME version",
        5 => "Downloading",
        _ if e.enabled => "On",
        _ => "Off",
    }
}

pub fn extensions_page(app: &mut App, ui: &mut Ui) {
    let pal = app.pal();
    let ctx = ui.ctx().clone();
    poll(app, &ctx);
    widgets::h1(ui, &pal, "Extensions");
    ui.label(
        RichText::new("Turn GNOME Shell extensions on or off, open their settings, add new ones or remove them.")
            .font(regular(12.5))
            .color(pal.text_faint),
    );
    if app.desk.extensions.is_none() && !app.desk.ext_loading {
        refresh_extensions(app, &ctx);
    }

    ui.add_space(14.0);
    let mut master = app.desk.user_extensions_on.unwrap_or(true);
    let mut install_zip = false;
    let mut refresh = false;
    widgets::card(ui, &pal, |ui| {
        if widgets::row(
            ui,
            &pal,
            pal.accent,
            Icon::Hub,
            "Use extensions",
            "Off turns every extension you installed off at once",
            Right::Switch(&mut master),
        )
        .toggled()
        {
            let ok = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.shell",
                    "disable-user-extensions",
                    if master { "false" } else { "true" },
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                app.desk.user_extensions_on = Some(master);
            }
        }
        widgets::divider(ui, &pal);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            install_zip = widgets::button(
                ui,
                &pal,
                pal.accent,
                "Install from a .zip file",
                Some(Icon::Apps),
            )
            .clicked();
            refresh = widgets::ghost_button(ui, &pal, "Refresh", None).clicked();
        });
        ui.add_space(10.0);
    });
    if refresh {
        refresh_extensions(app, &ctx);
    }
    if install_zip {
        spawn(app, &ctx, || {
            let picked = run(
                "zenity",
                &[
                    "--file-selection",
                    "--title=Choose an extension .zip",
                    "--file-filter=Extensions | *.zip",
                ],
            );
            Done::Message(match picked {
                Ok(path) if !path.is_empty() => {
                    run("gnome-extensions", &["install", "--force", &path])
                        .map(|_| "Extension installed — log out and back in to load it".to_owned())
                }
                Ok(_) => Ok(String::new()),
                // zenity exits 1 when the picker is closed: nothing to report.
                Err(e) if e == "failed" || e.is_empty() => Ok(String::new()),
                Err(e) => Err(format!("Could not open the file picker: {e}")),
            })
        });
    }

    // Get more from extensions.gnome.org (the shell asks before installing).
    section_title(ui, app, "Get extensions");
    let mut search = false;
    let mut install: Option<String> = None;
    widgets::card(ui, &pal, |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD);
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.desk.query)
                    .hint_text("Search extensions.gnome.org")
                    .desired_width(ui.available_width() - 130.0),
            );
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if app.desk.searching {
                ui.spinner();
            } else if widgets::button(ui, &pal, pal.accent, "Search", Some(Icon::Search)).clicked()
                || enter
            {
                search = !app.desk.query.trim().is_empty();
            }
        });
        ui.add_space(12.0);
        for found in &app.desk.found {
            widgets::divider(ui, &pal);
            let have = app
                .desk
                .extensions
                .as_ref()
                .is_some_and(|l| l.iter().any(|e| e.uuid == found.uuid));
            let right = if have {
                Right::AccentText("Installed")
            } else {
                Right::AccentText("Install")
            };
            let mut desc = found.description.clone();
            if desc.chars().count() > 90 {
                desc = desc
                    .chars()
                    .take(88)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
                    + "…";
            }
            if widgets::row(ui, &pal, pal.accent, Icon::Hub, &found.name, &desc, right).clicked()
                && !have
            {
                install = Some(found.uuid.clone());
            }
        }
    });
    if search {
        app.desk.searching = true;
        let q = app.desk.query.trim().to_owned();
        let shell = run("gnome-shell", &["--version"])
            .ok()
            .and_then(|v| {
                v.split_whitespace()
                    .last()
                    .map(|s| s.split('.').next().unwrap_or("").to_owned())
            })
            .unwrap_or_default();
        spawn(app, &ctx, move || {
            Done::Found(
                run("python3", &["-c", SEARCH_PY, &q, &shell]).and_then(|out| {
                    let v: Vec<serde_json::Value> =
                        serde_json::from_str(&out).map_err(|e| e.to_string())?;
                    Ok(v.iter()
                        .map(|e| Found {
                            uuid: e["uuid"].as_str().unwrap_or("").to_owned(),
                            name: e["name"].as_str().unwrap_or("").to_owned(),
                            description: e["description"].as_str().unwrap_or("").to_owned(),
                        })
                        .collect())
                }),
            )
        });
    }
    if let Some(uuid) = install {
        spawn(app, &ctx, move || {
            Done::Message(match run("python3", &["-c", REMOTE_PY, &uuid]).as_deref() {
                Ok("successful") => Ok("Extension installed".to_owned()),
                Ok("cancelled") => Ok("Install cancelled".to_owned()),
                Ok(other) => Err(format!("Not installed ({other})")),
                Err(e) => Err(e.to_string()),
            })
        });
    }

    let Some(list) = app.desk.extensions.clone() else {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Reading extensions…").color(pal.text_muted));
        });
        return;
    };
    let mut action: Option<(&'static str, String)> = None;
    let mut toggle_open: Option<Extension> = None;
    let mut set_opt: Option<(Extension, String, bool)> = None;
    for (title, user) in [("Installed by you", true), ("Built in", false)] {
        let rows: Vec<&Extension> = list.iter().filter(|e| e.user == user).collect();
        if rows.is_empty() {
            continue;
        }
        section_title(ui, app, title);
        widgets::card(ui, &pal, |ui| {
            for (n, e) in rows.iter().enumerate() {
                if n > 0 {
                    widgets::divider(ui, &pal);
                }
                let mut on = e.enabled;
                let mut sub = format!(
                    "{}{}{}",
                    state_text(e),
                    match (e.version.is_empty(), e.pending.is_empty()) {
                        (true, _) => String::new(),
                        (false, true) => format!(" · version {}", e.version),
                        (false, false) => format!(
                            " · version {} ({} after you sign out)",
                            e.version, e.pending
                        ),
                    },
                    if e.description.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", e.description)
                    }
                );
                if sub.chars().count() > 70 {
                    sub = sub
                        .chars()
                        .take(68)
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                        + "…";
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let extra = if e.has_prefs { 92.0 } else { 0.0 }
                        + if e.user { 92.0 } else { 0.0 }
                        + if e.has_prefs || e.user { ROW_PAD } else { 0.0 };
                    let row = ui.scope(|ui| {
                        ui.set_width(ui.available_width() - extra);
                        widgets::row(
                            ui,
                            &pal,
                            pal.accent,
                            Icon::Hub,
                            &e.name,
                            &sub,
                            Right::Switch(&mut on),
                        )
                    });
                    if row.inner.toggled() {
                        action = Some((if on { "enable" } else { "disable" }, e.uuid.clone()));
                    }
                    if e.has_prefs && widgets::ghost_button(ui, &pal, "Settings", None).clicked() {
                        action = Some(("prefs", e.uuid.clone()));
                    }
                    if e.user && widgets::ghost_button(ui, &pal, "Remove", None).clicked() {
                        action = Some(("uninstall", e.uuid.clone()));
                    }
                });
                if !e.schema.is_empty() && e.uuid.starts_with("fusion-desk@") {
                    let open = app.desk.open.contains(&e.uuid);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PAD + 42.0);
                        let label = ui.add(
                            egui::Label::new(
                                RichText::new(if open {
                                    "▾ Hide options"
                                } else {
                                    "▸ Options"
                                })
                                .font(regular(12.5))
                                .color(pal.accent),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if label
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            toggle_open = Some((*e).clone());
                        }
                    });
                    ui.add_space(6.0);
                    if open {
                        match app.desk.options.get(&e.uuid) {
                            None => {
                                ui.horizontal(|ui| {
                                    ui.add_space(ROW_PAD + 42.0);
                                    ui.spinner();
                                });
                            }
                            Some(opts) if opts.is_empty() => {
                                ui.horizontal(|ui| {
                                    ui.add_space(ROW_PAD + 42.0);
                                    ui.label(
                                        RichText::new("No on/off options")
                                            .font(regular(12.5))
                                            .color(pal.text_faint),
                                    );
                                });
                            }
                            Some(opts) => {
                                for o in opts {
                                    let mut on = o.on;
                                    if option_row(ui, &pal, &o.label, &mut on) {
                                        set_opt = Some(((*e).clone(), o.key.clone(), on));
                                    }
                                }
                                ui.add_space(6.0);
                            }
                        }
                    }
                }
            }
        });
    }
    if let Some(e) = toggle_open {
        if !app.desk.open.remove(&e.uuid) {
            app.desk.open.insert(e.uuid.clone());
            load_options(app, &ctx, &e, None);
        }
    }
    if let Some((e, key, on)) = set_opt {
        if let Some(list) = app.desk.options.get_mut(&e.uuid) {
            if let Some(o) = list.iter_mut().find(|o| o.key == key) {
                o.on = on;
            }
        }
        load_options(app, &ctx, &e, Some((key, on)));
    }
    if let Some((verb, uuid)) = action {
        if verb == "uninstall" {
            let name = list
                .iter()
                .find(|e| e.uuid == uuid)
                .map(|e| e.name.clone())
                .unwrap_or_default();
            app.confirm_extension_removal(uuid, name);
        } else {
            ext_action(app, &ctx, verb, uuid);
        }
    }
}

/// Called from the confirm dialog: uninstall a user extension.
pub fn uninstall(app: &mut App, ctx: &Context, uuid: String) {
    spawn(app, ctx, move || {
        Done::Message(
            run("gnome-extensions", &["uninstall", &uuid]).map(|_| "Extension removed".to_owned()),
        )
    });
}
