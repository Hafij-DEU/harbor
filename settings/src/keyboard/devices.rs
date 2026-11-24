//! Which keyboard is which. A keyboard is named in the config either as
//! `internal` (the laptop's own keyboard, on the i8042 bus) or by its USB/BT
//! `vvvv:pppp` vendor:product id, so a setting survives re-plugging.

/// The laptop's built-in keyboard sits on the i8042 (PS/2 controller) bus.
pub const BUS_I8042: u16 = 0x11;

/// Name of the virtual keyboard the service types on; never listed or grabbed.
pub const VIRTUAL_NAME: &str = "Harbor keymap";

/// Which keyboard(s) a rule or setting is for.
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash)]
pub enum Target {
    /// Every keyboard (config: no `on` part).
    #[default]
    Every,
    /// The laptop's own keyboard (config: `internal`).
    Internal,
    /// One keyboard by "vvvv:pppp" (config: the id itself).
    Id(String),
}

impl Target {
    /// Parse a config word: `internal` or `vvvv:pppp` (lower-case hex).
    pub fn parse(word: &str) -> Option<Target> {
        if word == "internal" {
            return Some(Target::Internal);
        }
        let b = word.as_bytes();
        let hex = |c: &u8| c.is_ascii_digit() || (b'a'..=b'f').contains(c);
        let ok = b.len() == 9 && b[4] == b':' && b[..4].iter().all(hex) && b[5..].iter().all(hex);
        ok.then(|| Target::Id(word.to_owned()))
    }

    /// Config word; `Every` has none (it is written as no `on` part).
    pub fn word(&self) -> Option<&str> {
        match self {
            Target::Every => None,
            Target::Internal => Some("internal"),
            Target::Id(id) => Some(id),
        }
    }

    /// Whether this target covers a keyboard with this id and bus.
    pub fn matches(&self, id: &str, bus: u16) -> bool {
        match self {
            Target::Every => true,
            Target::Internal => bus == BUS_I8042,
            Target::Id(want) => want == id,
        }
    }
}

/// A keyboard plugged in now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyboard {
    pub name: String,
    /// "vvvv:pppp"
    pub id: String,
    pub bus: u16,
}

impl Keyboard {
    /// How the config names this keyboard.
    pub fn target(&self) -> Target {
        if self.bus == BUS_I8042 {
            Target::Internal
        } else {
            Target::Id(self.id.clone())
        }
    }

    pub fn is_internal(&self) -> bool {
        self.bus == BUS_I8042
    }
}

/// Keyboards listed in /proc/bus/input/devices: the `kbd` handler plus key
/// repeat (power buttons and mice have no repeat). One entry per id.
/// Virtual keyboards (Harbor's own output, input-remapper's copies) are
/// left out: they only repeat a real keyboard.
pub fn keyboards() -> Vec<Keyboard> {
    let text = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
    parse_proc_devices(&text)
}

fn parse_proc_devices(text: &str) -> Vec<Keyboard> {
    let mut out: Vec<Keyboard> = Vec::new();
    for block in text.split("\n\n") {
        let (mut name, mut id, mut bus, mut kbd, mut rep, mut virt) =
            (String::new(), String::new(), 0, false, false, false);
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("I: ") {
                let field = |key: &str| {
                    rest.split_whitespace()
                        .find_map(|w| w.strip_prefix(key))
                        .unwrap_or("0000")
                        .to_ascii_lowercase()
                };
                bus = u16::from_str_radix(&field("Bus="), 16).unwrap_or(0);
                id = format!("{}:{}", field("Vendor="), field("Product="));
            } else if let Some(rest) = line.strip_prefix("N: Name=") {
                name = rest.trim_matches('"').to_owned();
            } else if let Some(rest) = line.strip_prefix("S: Sysfs=") {
                virt = rest.starts_with("/devices/virtual/");
            } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                kbd = rest.split_whitespace().any(|w| w == "kbd");
            } else if let Some(rest) = line.strip_prefix("B: EV=") {
                // EV_REP is bit 0x14.
                rep = u64::from_str_radix(rest.trim(), 16).is_ok_and(|ev| ev & (1 << 0x14) != 0);
            }
        }
        if kbd && rep && !virt && name != VIRTUAL_NAME && !out.iter().any(|k| k.id == id) {
            out.push(Keyboard { name, id, bus });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(Target::parse("internal"), Some(Target::Internal));
        assert_eq!(
            Target::parse("046d:c31c"),
            Some(Target::Id("046d:c31c".into()))
        );
        assert_eq!(Target::parse("046D:c31c"), None);
        assert_eq!(Target::parse("nowhere"), None);
        assert!(Target::Internal.matches("0001:0001", BUS_I8042));
        assert!(!Target::Internal.matches("0001:0001", 3));
        assert!(Target::Every.matches("x", 3));
    }

    #[test]
    fn proc_devices() {
        let text = "I: Bus=0011 Vendor=0001 Product=0001 Version=ab83\nN: Name=\"AT Translated Set 2 keyboard\"\nH: Handlers=sysrq kbd event3\nB: EV=120013\n\nI: Bus=0019 Vendor=0000 Product=0001 Version=0000\nN: Name=\"Power Button\"\nH: Handlers=kbd event1\nB: EV=3\n\nI: Bus=0011 Vendor=0001 Product=0001 Version=0001\nN: Name=\"input-remapper keyboard\"\nS: Sysfs=/devices/virtual/input/input14\nH: Handlers=sysrq kbd event13\nB: EV=120013\n\nI: Bus=0006 Vendor=1209 Product=4b4d Version=0001\nN: Name=\"Harbor keymap\"\nH: Handlers=sysrq kbd event9\nB: EV=100003\n";
        let k = parse_proc_devices(text);
        assert_eq!(k.len(), 1);
        assert!(k[0].is_internal());
        assert_eq!(k[0].target(), Target::Internal);
    }
}
