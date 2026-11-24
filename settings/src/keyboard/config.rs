//! `/etc/harbor/keyboard.conf`: everything the service does, one line each.
//!
//! ```text
//! remap on                              remap rules below are active (or `off`)
//! key CAPSLOCK ESC                      a key types another key or a shortcut
//! key F1 DISABLED on 046d:c31c          ... or nothing, on one keyboard only
//! stuck restart                         default for every keyboard when a key sticks
//! keyboard internal off                 the laptop keyboard is turned off
//! keyboard 046d:c31c stuck disable-key  this keyboard: own stuck-key action
//! ```
//!
//! Unknown or broken lines are skipped, so an old or hand-edited file never
//! stops the service. The root helper accepts only these line shapes (see
//! `packaging/keyboard/harbor-keyboard-apply`); keep both in step.

use super::devices::{Keyboard, Target};
use super::keys::{Combo, DISABLED};

pub const PATH: &str = "/etc/harbor/keyboard.conf";
/// Read when `PATH` does not exist yet (Settings 0.5.35 and older).
pub const OLD_PATH: &str = "/etc/harbor/keymap.conf";

/// One key types another key, a shortcut, or nothing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rule {
    /// Always one plain key (`mods == 0`).
    pub from: Combo,
    /// `to.key == DISABLED` makes the key do nothing.
    pub to: Combo,
    pub on: Target,
}

/// What the service does when a key is stuck down (see the service's `guard.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StuckAction {
    /// Do nothing.
    #[default]
    Nothing,
    /// Restart (reset) the keyboard; if the key sticks again within a minute,
    /// disable that key until the service restarts.
    Restart,
    /// Restart the keyboard and disable that key at once.
    DisableKey,
}

impl StuckAction {
    pub const ALL: [StuckAction; 3] = [
        StuckAction::Nothing,
        StuckAction::Restart,
        StuckAction::DisableKey,
    ];

    pub fn word(self) -> &'static str {
        match self {
            StuckAction::Nothing => "off",
            StuckAction::Restart => "restart",
            StuckAction::DisableKey => "disable-key",
        }
    }

    pub fn parse(word: &str) -> Option<StuckAction> {
        StuckAction::ALL.into_iter().find(|a| a.word() == word)
    }

    pub fn label(self) -> &'static str {
        match self {
            StuckAction::Nothing => "Do nothing",
            StuckAction::Restart => "Restart the keyboard",
            StuckAction::DisableKey => "Disable the stuck key",
        }
    }
}

/// Settings for one keyboard. Only written when something differs from the default.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BoardSettings {
    /// `Internal` or `Id`, never `Every`.
    pub target: Target,
    /// Turned off (the kernel ignores it).
    pub off: bool,
    /// Own stuck-key action; None follows the default.
    pub stuck: Option<StuckAction>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Config {
    pub remap: bool,
    pub rules: Vec<Rule>,
    /// Stuck-key action for keyboards without their own.
    pub stuck: StuckAction,
    pub boards: Vec<BoardSettings>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            remap: true,
            rules: Vec::new(),
            stuck: StuckAction::Nothing,
            boards: Vec::new(),
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Config {
        let mut cfg = Config::default();
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["remap", v] => cfg.remap = *v == "on",
                ["enabled", v] => cfg.remap = *v == "true", // old name of `remap`
                ["stuck", action] => {
                    if let Some(a) = StuckAction::parse(action) {
                        cfg.stuck = a;
                    }
                }
                ["keyboard", dev, rest @ ..] => {
                    if let Some(b) = parse_board(dev, rest) {
                        cfg.boards.retain(|x| x.target != b.target);
                        cfg.boards.push(b);
                    }
                }
                ["key", from, to, rest @ ..] => {
                    if let Some(r) = parse_rule(from, to, rest) {
                        cfg.rules.push(r);
                    }
                }
                _ => {}
            }
        }
        cfg
    }

    pub fn text(&self) -> String {
        let mut out = format!("remap {}\n", if self.remap { "on" } else { "off" });
        for r in &self.rules {
            out.push_str(&format!("key {} {}", r.from.text(), r.to.text()));
            if let Some(on) = r.on.word() {
                out.push_str(&format!(" on {on}"));
            }
            out.push('\n');
        }
        if self.stuck != StuckAction::Nothing {
            out.push_str(&format!("stuck {}\n", self.stuck.word()));
        }
        for b in &self.boards {
            let Some(dev) = b.target.word() else { continue };
            if !b.off && b.stuck.is_none() {
                continue;
            }
            out.push_str(&format!("keyboard {dev}"));
            if b.off {
                out.push_str(" off");
            }
            if let Some(a) = b.stuck {
                out.push_str(&format!(" stuck {}", a.word()));
            }
            out.push('\n');
        }
        out
    }

    /// The saved config (new path first, then the old one), or the default.
    pub fn load() -> Config {
        std::fs::read_to_string(PATH)
            .or_else(|_| std::fs::read_to_string(OLD_PATH))
            .map(|t| Config::parse(&t))
            .unwrap_or_default()
    }

    /// This keyboard's own settings, if any.
    pub fn board(&self, target: &Target) -> Option<&BoardSettings> {
        self.boards.iter().find(|b| &b.target == target)
    }

    /// This keyboard's settings, created with defaults when missing.
    pub fn board_mut(&mut self, target: &Target) -> &mut BoardSettings {
        if let Some(i) = self.boards.iter().position(|b| &b.target == target) {
            return &mut self.boards[i];
        }
        self.boards.push(BoardSettings {
            target: target.clone(),
            off: false,
            stuck: None,
        });
        self.boards.last_mut().expect("just pushed")
    }

    pub fn is_off(&self, kb: &Keyboard) -> bool {
        self.board(&kb.target()).is_some_and(|b| b.off)
    }

    /// The stuck-key action that applies to this keyboard.
    pub fn stuck_for(&self, kb: &Keyboard) -> StuckAction {
        self.board(&kb.target())
            .and_then(|b| b.stuck)
            .unwrap_or(self.stuck)
    }

    /// Remap rules that are active (remap on), one-keyboard rules first so
    /// they win over every-keyboard rules for the same key.
    pub fn active_rules(&self) -> Vec<Rule> {
        if !self.remap {
            return Vec::new();
        }
        let mut rules = self.rules.clone();
        rules.sort_by_key(|r| r.on == Target::Every);
        rules
    }
}

/// `keyboard DEV [off] [stuck ACTION]`
fn parse_board(dev: &str, rest: &[&str]) -> Option<BoardSettings> {
    let mut b = BoardSettings {
        target: Target::parse(dev)?,
        off: false,
        stuck: None,
    };
    let mut words = rest.iter();
    while let Some(w) = words.next() {
        match *w {
            "off" => b.off = true,
            "stuck" => b.stuck = Some(StuckAction::parse(words.next()?)?),
            _ => return None,
        }
    }
    Some(b)
}

/// `key FROM TO [on DEV]`
fn parse_rule(from: &str, to: &str, rest: &[&str]) -> Option<Rule> {
    let on = match rest {
        [] => Target::Every,
        ["on", dev] => Target::parse(dev)?,
        _ => return None,
    };
    let (from, to) = (Combo::parse(from)?, Combo::parse(to)?);
    // A rule starts from one plain key; nothing starts from "no key".
    if from.mods != 0 || from.key == DISABLED {
        return None;
    }
    Some(Rule { from, to, on })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_lines() {
        let text = "enabled true\nkey CAPSLOCK ESC\nshortcut CTRL+ALT+T SUPER+E\nkey E SUPER+E\nkey CTRL+A B\nkey NOPE ESC\nkey F1 DISABLED\nkey 102ND LEFTSHIFT on internal\nkey F2 F3 on 046d:c31c\nkey F4 F5 on nowhere\nstuck restart\nkeyboard internal off\nkeyboard 046d:c31c stuck disable-key\nkeyboard 046d:c31c bogus\n";
        let cfg = Config::parse(text);
        assert!(cfg.remap);
        assert_eq!(cfg.rules.len(), 5);
        assert_eq!(cfg.rules[3].on, Target::Internal);
        assert_eq!(cfg.rules[1].to.label(), "Super + E");
        assert_eq!(cfg.stuck, StuckAction::Restart);
        assert_eq!(cfg.boards.len(), 2);
        assert_eq!(Config::parse(&cfg.text()), cfg);
    }

    #[test]
    fn per_keyboard_settings() {
        let cfg =
            Config::parse("stuck restart\nkeyboard internal off\nkeyboard 046d:c31c stuck off\n");
        let laptop = Keyboard {
            name: "AT".into(),
            id: "0001:0001".into(),
            bus: 0x11,
        };
        let usb = Keyboard {
            name: "USB".into(),
            id: "046d:c31c".into(),
            bus: 3,
        };
        let other = Keyboard {
            name: "BT".into(),
            id: "05ac:0255".into(),
            bus: 5,
        };
        assert!(cfg.is_off(&laptop));
        assert!(!cfg.is_off(&usb));
        assert_eq!(cfg.stuck_for(&laptop), StuckAction::Restart);
        assert_eq!(cfg.stuck_for(&usb), StuckAction::Nothing);
        assert_eq!(cfg.stuck_for(&other), StuckAction::Restart);
    }

    #[test]
    fn one_keyboard_rules_first() {
        let cfg = Config::parse("remap on\nkey A B\nkey A C on internal\n");
        assert_eq!(cfg.active_rules()[0].on, Target::Internal);
        let off = Config::parse("remap off\nkey A B\n");
        assert!(off.active_rules().is_empty());
    }
}
