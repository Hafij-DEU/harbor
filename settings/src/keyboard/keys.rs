//! Key names, Linux key codes, the labels Settings shows, modifiers and
//! `Combo` (a key plus modifiers). Pure data, no I/O.

pub const MOD_CTRL: u8 = 1;
pub const MOD_ALT: u8 = 2;
pub const MOD_SHIFT: u8 = 4;
pub const MOD_SUPER: u8 = 8;

/// (bit, config name, left-side key code). Order is the order shown and written.
pub const MODS: [(u8, &str, u16); 4] = [
    (MOD_CTRL, "CTRL", 29),
    (MOD_ALT, "ALT", 56),
    (MOD_SHIFT, "SHIFT", 42),
    (MOD_SUPER, "SUPER", 125),
];

/// Code 0 means "no key": a disabled key in a remap rule.
pub const DISABLED: u16 = 0;

/// (name, Linux key code, label shown in Settings)
pub const KEYS: &[(&str, u16, &str)] = &[
    ("ESC", 1, "Esc"),
    ("1", 2, "1"),
    ("2", 3, "2"),
    ("3", 4, "3"),
    ("4", 5, "4"),
    ("5", 6, "5"),
    ("6", 7, "6"),
    ("7", 8, "7"),
    ("8", 9, "8"),
    ("9", 10, "9"),
    ("0", 11, "0"),
    ("MINUS", 12, "-"),
    ("EQUAL", 13, "="),
    ("BACKSPACE", 14, "Backspace"),
    ("TAB", 15, "Tab"),
    ("Q", 16, "Q"),
    ("W", 17, "W"),
    ("E", 18, "E"),
    ("R", 19, "R"),
    ("T", 20, "T"),
    ("Y", 21, "Y"),
    ("U", 22, "U"),
    ("I", 23, "I"),
    ("O", 24, "O"),
    ("P", 25, "P"),
    ("LEFTBRACE", 26, "["),
    ("RIGHTBRACE", 27, "]"),
    ("ENTER", 28, "Enter"),
    ("LEFTCTRL", 29, "Left Ctrl"),
    ("A", 30, "A"),
    ("S", 31, "S"),
    ("D", 32, "D"),
    ("F", 33, "F"),
    ("G", 34, "G"),
    ("H", 35, "H"),
    ("J", 36, "J"),
    ("K", 37, "K"),
    ("L", 38, "L"),
    ("SEMICOLON", 39, ";"),
    ("APOSTROPHE", 40, "'"),
    ("GRAVE", 41, "`"),
    ("LEFTSHIFT", 42, "Left Shift"),
    ("BACKSLASH", 43, "\\"),
    ("Z", 44, "Z"),
    ("X", 45, "X"),
    ("C", 46, "C"),
    ("V", 47, "V"),
    ("B", 48, "B"),
    ("N", 49, "N"),
    ("M", 50, "M"),
    ("COMMA", 51, ","),
    ("DOT", 52, "."),
    ("SLASH", 53, "/"),
    ("RIGHTSHIFT", 54, "Right Shift"),
    ("KPASTERISK", 55, "Num *"),
    ("LEFTALT", 56, "Left Alt"),
    ("SPACE", 57, "Space"),
    ("CAPSLOCK", 58, "Caps Lock"),
    ("F1", 59, "F1"),
    ("F2", 60, "F2"),
    ("F3", 61, "F3"),
    ("F4", 62, "F4"),
    ("F5", 63, "F5"),
    ("F6", 64, "F6"),
    ("F7", 65, "F7"),
    ("F8", 66, "F8"),
    ("F9", 67, "F9"),
    ("F10", 68, "F10"),
    ("NUMLOCK", 69, "Num Lock"),
    ("SCROLLLOCK", 70, "Scroll Lock"),
    ("KP7", 71, "Num 7"),
    ("KP8", 72, "Num 8"),
    ("KP9", 73, "Num 9"),
    ("KPMINUS", 74, "Num -"),
    ("KP4", 75, "Num 4"),
    ("KP5", 76, "Num 5"),
    ("KP6", 77, "Num 6"),
    ("KPPLUS", 78, "Num +"),
    ("KP1", 79, "Num 1"),
    ("KP2", 80, "Num 2"),
    ("KP3", 81, "Num 3"),
    ("KP0", 82, "Num 0"),
    ("KPDOT", 83, "Num ."),
    ("102ND", 86, "< > (102nd key)"),
    ("F11", 87, "F11"),
    ("F12", 88, "F12"),
    ("KPENTER", 96, "Num Enter"),
    ("RIGHTCTRL", 97, "Right Ctrl"),
    ("KPSLASH", 98, "Num /"),
    ("SYSRQ", 99, "Print Screen"),
    ("RIGHTALT", 100, "Right Alt"),
    ("HOME", 102, "Home"),
    ("UP", 103, "Up"),
    ("PAGEUP", 104, "Page Up"),
    ("LEFT", 105, "Left"),
    ("RIGHT", 106, "Right"),
    ("END", 107, "End"),
    ("DOWN", 108, "Down"),
    ("PAGEDOWN", 109, "Page Down"),
    ("INSERT", 110, "Insert"),
    ("DELETE", 111, "Delete"),
    ("MUTE", 113, "Mute"),
    ("VOLUMEDOWN", 114, "Volume Down"),
    ("VOLUMEUP", 115, "Volume Up"),
    ("PAUSE", 119, "Pause"),
    ("LEFTMETA", 125, "Left Super"),
    ("RIGHTMETA", 126, "Right Super"),
    ("COMPOSE", 127, "Menu"),
    ("CALC", 140, "Calculator"),
    ("MAIL", 155, "Mail"),
    ("NEXTSONG", 163, "Next Track"),
    ("PLAYPAUSE", 164, "Play/Pause"),
    ("PREVIOUSSONG", 165, "Previous Track"),
    ("HOMEPAGE", 172, "Browser Home"),
    ("F13", 183, "F13"),
    ("F14", 184, "F14"),
    ("F15", 185, "F15"),
    ("F16", 186, "F16"),
    ("F17", 187, "F17"),
    ("F18", 188, "F18"),
    ("F19", 189, "F19"),
    ("F20", 190, "F20"),
    ("F21", 191, "F21"),
    ("F22", 192, "F22"),
    ("F23", 193, "F23"),
    ("F24", 194, "F24"),
    ("BRIGHTNESSDOWN", 224, "Brightness Down"),
    ("BRIGHTNESSUP", 225, "Brightness Up"),
    ("DISABLED", 0, "Disabled"),
];

/// Code for a config name ("CAPSLOCK" -> 58).
pub fn code(name: &str) -> Option<u16> {
    KEYS.iter().find(|k| k.0 == name).map(|k| k.1)
}
/// Config name of a code ("CAPSLOCK"); unknown codes read as "DISABLED".
pub fn name(code: u16) -> &'static str {
    KEYS.iter()
        .find(|k| k.1 == code)
        .map_or("DISABLED", |k| k.0)
}

/// Settings label of a code ("Caps Lock"); unknown codes read as "Disabled".
pub fn label(code: u16) -> &'static str {
    KEYS.iter()
        .find(|k| k.1 == code)
        .map_or("Disabled", |k| k.2)
}

/// Which modifier a key code is (left and right both count), or 0.
pub fn mod_bit(code: u16) -> u8 {
    match code {
        29 | 97 => MOD_CTRL,
        56 | 100 => MOD_ALT,
        42 | 54 => MOD_SHIFT,
        125 | 126 => MOD_SUPER,
        _ => 0,
    }
}

/// A label for any code, also ones missing from `KEYS` ("Key 240").
pub fn any_label(code: u16) -> String {
    match KEYS.iter().find(|k| k.1 == code) {
        Some(k) if code != DISABLED => k.2.to_owned(),
        _ => format!("Key {code}"),
    }
}

/// Short modifier label: "Ctrl", "Alt", "Shift", "Super".
pub fn mod_label(bit: u8) -> &'static str {
    match bit {
        MOD_CTRL => "Ctrl",
        MOD_ALT => "Alt",
        MOD_SHIFT => "Shift",
        _ => "Super",
    }
}

/// A key with modifiers. `key` 0 (`DISABLED`) means "do nothing".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Combo {
    pub mods: u8,
    pub key: u16,
}

impl Combo {
    /// "CTRL+ALT+T" -> Combo. None for unknown names.
    pub fn parse(text: &str) -> Option<Combo> {
        let parts: Vec<&str> = text.split('+').collect();
        let (last, mods) = parts.split_last()?;
        let mut combo = Combo::default();
        for m in mods {
            combo.mods |= MODS.iter().find(|x| x.1 == *m)?.0;
        }
        combo.key = code(last)?;
        Some(combo)
    }

    /// Config form: "CTRL+ALT+T".
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (bit, name, _) in MODS {
            if self.mods & bit != 0 {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push_str(name(self.key));
        out
    }

    /// Display form: "Ctrl + Alt + T".
    pub fn label(&self) -> String {
        let mut out = String::new();
        for (bit, _, _) in MODS {
            if self.mods & bit != 0 {
                out.push_str(mod_label(bit));
                out.push_str(" + ");
            }
        }
        out.push_str(label(self.key));
        out
    }
}
