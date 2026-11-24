//! Files the service writes for Settings to read, under `/run/harbor-keyboard/`.
//!
//! * `status` (0644): each keyboard's state, keys disabled after sticking,
//!   and the latest stuck-key events. Holds no typed keys.
//! * `keycheck` (0600, owned by the active user): keys held and pressed
//!   right now. Written only while Settings asks for it by touching
//!   `/run/user/<uid>/harbor-keycheck` at least every `KEYCHECK_FRESH`;
//!   removed as soon as the asking stops.

use std::path::PathBuf;
use std::time::Duration;

pub const DIR: &str = "/run/harbor-keyboard";
pub const STATUS: &str = "/run/harbor-keyboard/status";
pub const KEYCHECK: &str = "/run/harbor-keyboard/keycheck";
/// The service drops Key check when the request file is older than this.
pub const KEYCHECK_FRESH: Duration = Duration::from_secs(3);

/// The file Settings touches to ask for Key check data.
pub fn keycheck_request(uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}/harbor-keycheck"))
}

/// One keyboard as the service sees it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct BoardStatus {
    pub id: String,
    pub bus: u16,
    pub name: String,
    /// Turned off by the service.
    pub off: bool,
    /// Set to off, but kept on because it is the only keyboard left.
    pub kept_on: bool,
    /// Keys disabled after they got stuck (until the service restarts).
    pub disabled: Vec<u16>,
}

/// A stuck key the service acted on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StuckEvent {
    /// Unix seconds.
    pub at: u64,
    pub id: String,
    pub code: u16,
    /// "restart" or "disable-key".
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Status {
    pub boards: Vec<BoardStatus>,
    pub events: Vec<StuckEvent>,
}

impl Status {
    /// Lines: `board ID BUS OFF KEPT_ON DISABLED NAME...` (DISABLED is
    /// comma-separated codes or `-`) and `event AT ID CODE ACTION`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for b in &self.boards {
            let disabled = if b.disabled.is_empty() {
                "-".to_owned()
            } else {
                b.disabled
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            out.push_str(&format!(
                "board {} {:04x} {} {} {} {}\n",
                b.id, b.bus, b.off as u8, b.kept_on as u8, disabled, b.name
            ));
        }
        for e in &self.events {
            out.push_str(&format!(
                "event {} {} {} {}\n",
                e.at, e.id, e.code, e.action
            ));
        }
        out
    }

    pub fn parse(text: &str) -> Status {
        let mut s = Status::default();
        for line in text.lines() {
            let w: Vec<&str> = line.splitn(7, ' ').collect();
            match w.as_slice() {
                ["board", id, bus, off, kept, disabled, name] => s.boards.push(BoardStatus {
                    id: (*id).to_owned(),
                    bus: u16::from_str_radix(bus, 16).unwrap_or(0),
                    name: (*name).to_owned(),
                    off: *off == "1",
                    kept_on: *kept == "1",
                    disabled: disabled.split(',').filter_map(|c| c.parse().ok()).collect(),
                }),
                ["event", at, id, code, action] => s.events.push(StuckEvent {
                    at: at.parse().unwrap_or(0),
                    id: (*id).to_owned(),
                    code: code.parse().unwrap_or(0),
                    action: (*action).to_owned(),
                }),
                _ => {}
            }
        }
        s
    }

    /// The current status, or None when the service is not running.
    pub fn read() -> Option<Status> {
        std::fs::read_to_string(STATUS)
            .ok()
            .map(|t| Status::parse(&t))
    }
}

/// Live keys for Key check.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct KeyCheck {
    /// (keyboard id, code, milliseconds held)
    pub held: Vec<(String, u16, u64)>,
    /// Latest presses, newest last: (unix milliseconds, keyboard id, code)
    pub presses: Vec<(u64, String, u16)>,
}

impl KeyCheck {
    /// Lines: `held ID CODE MS` and `press AT_MS ID CODE`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (id, code, ms) in &self.held {
            out.push_str(&format!("held {id} {code} {ms}\n"));
        }
        for (at, id, code) in &self.presses {
            out.push_str(&format!("press {at} {id} {code}\n"));
        }
        out
    }

    pub fn parse(text: &str) -> KeyCheck {
        let mut k = KeyCheck::default();
        for line in text.lines() {
            match line.split(' ').collect::<Vec<_>>().as_slice() {
                ["held", id, code, ms] => {
                    if let (Ok(c), Ok(m)) = (code.parse(), ms.parse()) {
                        k.held.push(((*id).to_owned(), c, m));
                    }
                }
                ["press", at, id, code] => {
                    if let (Ok(a), Ok(c)) = (at.parse(), code.parse()) {
                        k.presses.push((a, (*id).to_owned(), c));
                    }
                }
                _ => {}
            }
        }
        k
    }

    pub fn read() -> Option<KeyCheck> {
        std::fs::read_to_string(KEYCHECK)
            .ok()
            .map(|t| KeyCheck::parse(&t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = Status {
            boards: vec![BoardStatus {
                id: "0001:0001".into(),
                bus: 0x11,
                name: "AT Translated Set 2 keyboard".into(),
                off: false,
                kept_on: true,
                disabled: vec![42, 54],
            }],
            events: vec![StuckEvent {
                at: 5,
                id: "0001:0001".into(),
                code: 42,
                action: "restart".into(),
            }],
        };
        assert_eq!(Status::parse(&s.text()), s);
        let k = KeyCheck {
            held: vec![("046d:c31c".into(), 30, 1200)],
            presses: vec![(99, "0001:0001".into(), 58)],
        };
        assert_eq!(KeyCheck::parse(&k.text()), k);
    }
}
