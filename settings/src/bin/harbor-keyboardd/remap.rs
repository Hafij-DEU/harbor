//! The remap engine: turns key events from grabbed keyboards into events on
//! the virtual keyboard, applying `key` and `shortcut` rules. Keys without a
//! rule pass through unchanged.

use std::collections::{HashMap, HashSet};

use crate::evdev::VirtualKeyboard;
use crate::keyboard::config::{Rule, RuleKind};
use crate::keyboard::keys::{mod_bit, Combo, DISABLED, MODS};

/// What a pressed source key turned into, to undo on release.
struct Active {
    to: Combo,
    /// Modifiers pressed only for this rule's output.
    added_mods: Vec<u16>,
    /// Modifiers lifted for a shortcut, pressed again on release if still held.
    lifted: Vec<u16>,
}

pub struct Remapper {
    out: VirtualKeyboard,
    /// Active rules, one-keyboard rules first (see `Config::active_rules`).
    rules: Vec<Rule>,
    /// Keys down on the virtual keyboard.
    down: HashSet<u16>,
    /// Source key -> what it produced.
    active: HashMap<u16, Active>,
}

impl Remapper {
    pub fn new(out: VirtualKeyboard, rules: Vec<Rule>) -> Remapper {
        Remapper {
            out,
            rules,
            down: HashSet::new(),
            active: HashMap::new(),
        }
    }

    pub fn set_rules(&mut self, rules: Vec<Rule>) {
        self.rules = rules;
    }

    /// No key is down on the virtual keyboard (safe to change rules or grabs).
    pub fn idle(&self) -> bool {
        self.down.is_empty() && self.active.is_empty()
    }

    /// One key event from keyboard `id` on `bus`. value: 1 press, 2 repeat, 0 release.
    pub fn key(&mut self, code: u16, value: i32, id: &str, bus: u16) {
        match value {
            1 => self.press_source(code, id, bus),
            2 => match self.active.get(&code).map(|a| a.to.key) {
                Some(DISABLED) => {}
                Some(key) => self.emit(key, 2),
                None => self.emit(code, 2),
            },
            _ => self.release_source(code),
        }
    }

    fn press_source(&mut self, code: u16, id: &str, bus: u16) {
        let mods = self.mods();
        let shortcut = (mod_bit(code) == 0)
            .then(|| {
                self.rules.iter().find(|r| {
                    r.kind == RuleKind::Shortcut
                        && r.from.key == code
                        && r.from.mods == mods
                        && r.on.matches(id, bus)
                })
            })
            .flatten()
            .map(|r| r.to);
        if let Some(to) = shortcut {
            // Lift the held modifiers so only the target shortcut is seen.
            let lifted: Vec<u16> = self
                .down
                .iter()
                .copied()
                .filter(|&c| mod_bit(c) != 0)
                .collect();
            for &m in &lifted {
                self.emit(m, 0);
            }
            let added_mods = self.press_combo(to);
            self.active.insert(
                code,
                Active {
                    to,
                    added_mods,
                    lifted,
                },
            );
            return;
        }
        let key_rule = self
            .rules
            .iter()
            .find(|r| r.kind == RuleKind::Key && r.from.key == code && r.on.matches(id, bus))
            .map(|r| r.to);
        if let Some(to) = key_rule {
            let added_mods = self.press_combo(to);
            self.active.insert(
                code,
                Active {
                    to,
                    added_mods,
                    lifted: Vec::new(),
                },
            );
        } else {
            self.emit(code, 1);
        }
    }

    fn release_source(&mut self, code: u16) {
        if let Some(a) = self.active.remove(&code) {
            if a.to.key != DISABLED {
                self.emit(a.to.key, 0);
            }
            for m in a.added_mods {
                self.emit(m, 0);
            }
            for m in a.lifted {
                self.emit(m, 1);
            }
            return;
        }
        // A modifier let go while its shortcut is still down.
        for a in self.active.values_mut() {
            a.lifted.retain(|&m| m != code);
        }
        if self.down.contains(&code) {
            self.emit(code, 0);
        }
    }

    /// Press the modifiers a combo needs (if not down yet), then its key.
    fn press_combo(&mut self, to: Combo) -> Vec<u16> {
        let mut added = Vec::new();
        for (bit, _, code) in MODS {
            if to.mods & bit != 0 && self.mods() & bit == 0 {
                self.emit(code, 1);
                added.push(code);
            }
        }
        if to.key != DISABLED {
            self.emit(to.key, 1);
        }
        added
    }

    /// Modifier bits down on the virtual keyboard.
    fn mods(&self) -> u8 {
        self.down.iter().fold(0, |m, &c| m | mod_bit(c))
    }

    fn emit(&mut self, code: u16, value: i32) {
        self.out.key(code, value);
        match value {
            1 => {
                self.down.insert(code);
            }
            0 => {
                self.down.remove(&code);
            }
            _ => {}
        }
    }
}
