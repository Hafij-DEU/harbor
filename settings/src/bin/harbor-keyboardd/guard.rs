//! Stuck-key guard: per-keyboard bookkeeping that spots a stuck key.
//!
//! A key is stuck when the kernel still holds it down but the keyboard has
//! gone silent: a lost "key released" message. How we know it is silent:
//! * laptop keyboard (i8042): its interrupt counter stops. A key that is
//!   really held keeps sending repeats, so 4 seconds of silence is enough
//!   (same rule as Toggle Internal Keyboard's watchdog);
//! * other keyboards (USB, Bluetooth) send nothing while a key is held, so
//!   held and stuck look alike; we wait 30 seconds of silence to be safe.
//!
//! Pure logic: the service feeds it events and the time, and acts on what
//! `stuck_key` and `decide` return.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::keyboard::config::StuckAction;

// Default times; the real ones come from the config's `stuck-time` line.
#[cfg(test)]
pub const LIMIT_LAPTOP: Duration = Duration::from_secs(4);
#[cfg(test)]
pub const LIMIT_OTHER: Duration = Duration::from_secs(30);
/// A key stuck again this soon after a restart gets disabled instead.
#[cfg(test)]
pub const AGAIN_WITHIN: Duration = Duration::from_secs(60);

/// What to do about a stuck key now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    Restart,
    DisableKey,
}

#[derive(Default)]
pub struct Watch {
    /// Key -> when it went down (press/release events, repeats ignored).
    down_since: HashMap<u16, Instant>,
    /// Last sign of life from the hardware.
    last_activity: Option<Instant>,
    /// Last time the guard restarted this keyboard.
    last_restart: Option<Instant>,
}

impl Watch {
    /// A key event (value 1 press, 2 repeat, 0 release).
    pub fn event(&mut self, code: u16, value: i32, now: Instant) {
        match value {
            1 => {
                self.down_since.entry(code).or_insert(now);
                self.last_activity = Some(now);
            }
            0 => {
                self.down_since.remove(&code);
                self.last_activity = Some(now);
            }
            // Repeats can come from the kernel's own timer, so they prove nothing.
            _ => {}
        }
    }

    /// The hardware sent something (the i8042 interrupt counter moved).
    pub fn activity(&mut self, now: Instant) {
        self.last_activity = Some(now);
    }

    /// How long a key has been down, if we saw it go down.
    pub fn held_for(&self, code: u16, now: Instant) -> Option<Duration> {
        self.down_since.get(&code).map(|t| now.duration_since(*t))
    }

    /// The first key that is stuck: down at least `limit` according to our
    /// events AND to the kernel (`kernel_held`), with `limit` of silence.
    /// Keys in `ignore` (already disabled) are skipped.
    pub fn stuck_key(
        &self,
        now: Instant,
        limit: Duration,
        kernel_held: &[u16],
        ignore: &[u16],
    ) -> Option<u16> {
        let silent = self
            .last_activity
            .is_none_or(|t| now.duration_since(t) >= limit);
        if !silent {
            return None;
        }
        let mut keys: Vec<u16> = self
            .down_since
            .iter()
            .filter(|(code, since)| {
                now.duration_since(**since) >= limit
                    && kernel_held.contains(code)
                    && !ignore.contains(code)
            })
            .map(|(code, _)| *code)
            .collect();
        keys.sort_unstable();
        keys.first().copied()
    }

    /// Turn the keyboard's stuck-key action into what to do now.
    /// `Nothing` never reaches here. Stuck again within `again` of a restart
    /// disables the key; a zero `again` never does.
    pub fn decide(&mut self, action: StuckAction, now: Instant, again: Duration) -> Decision {
        match action {
            StuckAction::DisableKey => Decision::DisableKey,
            _ => {
                let soon = self
                    .last_restart
                    .is_some_and(|t| now.duration_since(t) < again);
                if soon {
                    self.last_restart = None;
                    Decision::DisableKey
                } else {
                    self.last_restart = Some(now);
                    Decision::Restart
                }
            }
        }
    }

    /// After a restart the kernel released everything.
    pub fn clear(&mut self, now: Instant) {
        self.down_since.clear();
        self.last_activity = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: u16 = 42;

    #[test]
    fn silent_held_key_is_stuck() {
        let t0 = Instant::now();
        let mut w = Watch::default();
        w.event(SHIFT, 1, t0);
        let t = t0 + Duration::from_secs(5);
        assert_eq!(w.stuck_key(t, LIMIT_LAPTOP, &[SHIFT], &[]), Some(SHIFT));
        // The kernel no longer holds it: not stuck.
        assert_eq!(w.stuck_key(t, LIMIT_LAPTOP, &[], &[]), None);
        // Already disabled: ignored.
        assert_eq!(w.stuck_key(t, LIMIT_LAPTOP, &[SHIFT], &[SHIFT]), None);
    }

    #[test]
    fn held_key_with_activity_is_not_stuck() {
        let t0 = Instant::now();
        let mut w = Watch::default();
        w.event(SHIFT, 1, t0);
        w.activity(t0 + Duration::from_secs(3)); // typematic repeats keep coming
        let t = t0 + Duration::from_secs(5);
        assert_eq!(w.stuck_key(t, LIMIT_LAPTOP, &[SHIFT], &[]), None);
        w.event(SHIFT, 0, t);
        assert_eq!(
            w.stuck_key(t + LIMIT_OTHER, LIMIT_LAPTOP, &[SHIFT], &[]),
            None
        );
    }

    #[test]
    fn restart_then_disable_when_it_sticks_again() {
        let t0 = Instant::now();
        let mut w = Watch::default();
        assert_eq!(
            w.decide(StuckAction::Restart, t0, AGAIN_WITHIN),
            Decision::Restart
        );
        assert_eq!(
            w.decide(
                StuckAction::Restart,
                t0 + Duration::from_secs(10),
                AGAIN_WITHIN
            ),
            Decision::DisableKey
        );
        assert_eq!(
            w.decide(
                StuckAction::Restart,
                t0 + Duration::from_secs(20),
                AGAIN_WITHIN
            ),
            Decision::Restart
        );
        assert_eq!(
            w.decide(
                StuckAction::Restart,
                t0 + Duration::from_secs(200),
                AGAIN_WITHIN
            ),
            Decision::Restart
        );
        assert_eq!(
            w.decide(StuckAction::DisableKey, t0, AGAIN_WITHIN),
            Decision::DisableKey
        );
        // A zero window never disables: every stuck key only restarts.
        let mut n = Watch::default();
        assert_eq!(
            n.decide(StuckAction::Restart, t0, Duration::ZERO),
            Decision::Restart
        );
        assert_eq!(
            n.decide(
                StuckAction::Restart,
                t0 + Duration::from_secs(1),
                Duration::ZERO
            ),
            Decision::Restart
        );
    }
}
