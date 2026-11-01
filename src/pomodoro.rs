//! Pomodoro mode: 25 minutes of work with the PC free, then the screen locks for a 5-minute
//! break; every 4th break lasts 15 minutes. It goes on until it's stopped.
//!
//! The whole schedule follows from the moment the Pomodoro started (a Unix time, in
//! milliseconds). Two paired devices that know the same start time are therefore always in the
//! same round and lock at the same moment, even if one of them joins halfway or wakes up from
//! sleep. A pause freezes the schedule; resuming moves the start later by the pause's length.
//! main.rs ticks a `Tracker` every second, shows the reminder and starts the breaks.

use std::time::Duration;

pub const WORK: Duration = Duration::from_secs(25 * 60);
pub const SHORT_BREAK: Duration = Duration::from_secs(5 * 60);
pub const LONG_BREAK: Duration = Duration::from_secs(15 * 60);
/// Work rounds per cycle; the break after the last one is the long one
pub const ROUNDS: u32 = 4;
/// How long before a break the reminder appears
pub const REMINDER: Duration = Duration::from_secs(60);
/// A break that has less than this left (e.g. when joining at its very end) isn't worth locking for
const MIN_BREAK: Duration = Duration::from_secs(5);

/// The break that follows work round `round`
pub fn break_length(round: u32) -> Duration {
    if round == ROUNDS { LONG_BREAK } else { SHORT_BREAK }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Work { round: u32, remaining: Duration },
    Break { round: u32, remaining: Duration },
}

impl Phase {
    pub fn round(&self) -> u32 {
        match *self {
            Phase::Work { round, .. } | Phase::Break { round, .. } => round,
        }
    }
}

/// Where a Pomodoro started at `start_ms` is at `now_ms`
pub fn phase_at(start_ms: u64, now_ms: u64) -> Phase {
    let cycle: Duration = WORK * ROUNDS + SHORT_BREAK * (ROUNDS - 1) + LONG_BREAK;
    let elapsed = now_ms.saturating_sub(start_ms) % cycle.as_millis() as u64;
    let mut t = Duration::from_millis(elapsed);
    for round in 1..=ROUNDS {
        if t < WORK {
            return Phase::Work { round, remaining: WORK - t };
        }
        t -= WORK;
        let length = break_length(round);
        if t < length {
            return Phase::Break { round, remaining: length - t };
        }
        t -= length;
    }
    // Only reached by rounding at the very end of a cycle
    Phase::Work { round: 1, remaining: WORK }
}

/// What a tick asks main.rs to do
#[derive(Debug, PartialEq)]
pub enum Event {
    /// A minute left: time to save your work
    Remind,
    /// A break has begun: lock the screen for this long
    BreakDue(Duration),
}

/// Follows a running Pomodoro and reports when something has to happen
pub struct Tracker {
    start_ms: u64,
    // When it was paused, while it is
    paused_at: Option<u64>,
    // The phase seen at the last tick, as (round, is a break)
    last: Option<(u32, bool)>,
    reminded: bool,
}

impl Tracker {
    pub fn new(start_ms: u64, paused_at: Option<u64>) -> Self {
        Self { start_ms, paused_at, last: None, reminded: false }
    }

    pub fn start_ms(&self) -> u64 {
        self.start_ms
    }

    pub fn paused_at(&self) -> Option<u64> {
        self.paused_at
    }

    /// While paused, the phase stays where it was when the pause began
    pub fn phase(&self, now_ms: u64) -> Phase {
        phase_at(self.start_ms, self.paused_at.unwrap_or(now_ms))
    }

    pub fn pause(&mut self, now_ms: u64) {
        self.paused_at.get_or_insert(now_ms);
    }

    pub fn resume(&mut self, now_ms: u64) {
        if let Some(paused_at) = self.paused_at.take() {
            self.start_ms += now_ms.saturating_sub(paused_at);
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Option<Event> {
        if self.paused_at.is_some() {
            return None;
        }
        let phase = self.phase(now_ms);
        let key = (phase.round(), matches!(phase, Phase::Break { .. }));
        let entered = self.last.replace(key) != Some(key);
        match phase {
            // Only when the break begins: after an emergency exit the screen stays unlocked
            Phase::Break { remaining, .. } if entered && remaining >= MIN_BREAK => Some(Event::BreakDue(remaining)),
            Phase::Break { .. } => None,
            Phase::Work { remaining, .. } => {
                if entered {
                    self.reminded = false;
                }
                if remaining <= REMINDER && !self.reminded {
                    self.reminded = true;
                    Some(Event::Remind)
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(d: Duration) -> u64 {
        d.as_millis() as u64
    }

    #[test]
    fn follows_the_schedule() {
        let start = 1_000_000;
        assert_eq!(phase_at(start, start), Phase::Work { round: 1, remaining: WORK });
        assert_eq!(phase_at(start, start + ms(WORK)), Phase::Break { round: 1, remaining: SHORT_BREAK });
        let fourth_break = start + ms(WORK * 4 + SHORT_BREAK * 3);
        assert_eq!(phase_at(start, fourth_break), Phase::Break { round: 4, remaining: LONG_BREAK });
        // Back to round 1 after the long break
        assert_eq!(phase_at(start, fourth_break + ms(LONG_BREAK)), Phase::Work { round: 1, remaining: WORK });
    }

    #[test]
    fn reminds_once_then_locks_once() {
        let start = 0;
        let mut tracker = Tracker::new(start, None);
        assert_eq!(tracker.tick(start), None);
        assert_eq!(tracker.tick(ms(WORK - REMINDER)), Some(Event::Remind));
        assert_eq!(tracker.tick(ms(WORK - REMINDER) + 1000), None);
        assert_eq!(tracker.tick(ms(WORK)), Some(Event::BreakDue(SHORT_BREAK)));
        // Still the same break (e.g. after an emergency exit): no second lock
        assert_eq!(tracker.tick(ms(WORK) + 10_000), None);
        // Next round reminds again
        assert_eq!(tracker.tick(ms(WORK * 2 + SHORT_BREAK - REMINDER)), Some(Event::Remind));
    }

    #[test]
    fn joining_during_a_break_locks_for_what_is_left() {
        let mut tracker = Tracker::new(0, None);
        let now = ms(WORK) + 60_000;
        assert_eq!(tracker.tick(now), Some(Event::BreakDue(SHORT_BREAK - Duration::from_secs(60))));
        // Too late to bother
        let mut tracker = Tracker::new(0, None);
        assert_eq!(tracker.tick(ms(WORK + SHORT_BREAK) - 2000), None);
    }

    #[test]
    fn a_pause_freezes_the_countdown() {
        let mut tracker = Tracker::new(0, None);
        tracker.tick(0);
        tracker.pause(ms(WORK) - 300_000);
        // Long past the end of the work round, but paused: no break
        assert_eq!(tracker.tick(ms(WORK) * 3), None);
        assert_eq!(tracker.phase(ms(WORK) * 3), Phase::Work { round: 1, remaining: Duration::from_secs(300) });
        tracker.resume(ms(WORK) * 3);
        assert_eq!(tracker.phase(ms(WORK) * 3), Phase::Work { round: 1, remaining: Duration::from_secs(300) });
        assert_eq!(tracker.tick(ms(WORK) * 3 + 300_000), Some(Event::BreakDue(SHORT_BREAK)));
    }
}
