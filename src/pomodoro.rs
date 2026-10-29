//! Pomodoro mode: 25 minutes of work with the PC free, then the screen locks for a 5-minute
//! break; every 4th break lasts 15 minutes. It goes on until it's stopped.
//!
//! This file only keeps track of the cycle. main.rs ticks it every second, shows the reminder
//! and starts the breaks.

use std::time::{Duration, Instant};

pub const WORK: Duration = Duration::from_secs(25 * 60);
pub const SHORT_BREAK: Duration = Duration::from_secs(5 * 60);
pub const LONG_BREAK: Duration = Duration::from_secs(15 * 60);
/// Work rounds per cycle; the break after the last one is the long one
pub const ROUNDS: u32 = 4;
/// How long before a break the reminder appears
pub const REMINDER: Duration = Duration::from_secs(60);

/// What a tick asks main.rs to do
#[derive(Debug, PartialEq)]
pub enum Event {
    /// A minute left: time to save your work
    Remind,
    /// Work is over: lock the screen for this long
    BreakDue(Duration),
}

pub struct Pomodoro {
    /// The current work round, 1 to ROUNDS (during a break, the round just finished)
    round: u32,
    /// When the current work round ends; None during a break
    work_end: Option<Instant>,
    reminded: bool,
}

impl Pomodoro {
    pub fn start(now: Instant) -> Self {
        Self { round: 1, work_end: Some(now + WORK), reminded: false }
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    /// The break that follows the current round
    pub fn break_length(&self) -> Duration {
        if self.round == ROUNDS { LONG_BREAK } else { SHORT_BREAK }
    }

    /// Work time left in the current round; None during a break
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.work_end.map(|end| end.saturating_duration_since(now))
    }

    pub fn tick(&mut self, now: Instant) -> Option<Event> {
        let remaining = self.remaining(now)?;
        if remaining.is_zero() {
            self.work_end = None;
            Some(Event::BreakDue(self.break_length()))
        } else if remaining <= REMINDER && !self.reminded {
            self.reminded = true;
            Some(Event::Remind)
        } else {
            None
        }
    }

    /// The break has ended: on to the next work round (back to round 1 after the long break)
    pub fn break_over(&mut self, now: Instant) {
        self.round = self.round % ROUNDS + 1;
        self.work_end = Some(now + WORK);
        self.reminded = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminds_once_then_asks_for_a_break() {
        let start = Instant::now();
        let mut pomodoro = Pomodoro::start(start);
        assert_eq!(pomodoro.tick(start + Duration::from_secs(60)), None);
        assert_eq!(pomodoro.tick(start + WORK - REMINDER), Some(Event::Remind));
        assert_eq!(pomodoro.tick(start + WORK - Duration::from_secs(30)), None);
        assert_eq!(pomodoro.tick(start + WORK), Some(Event::BreakDue(SHORT_BREAK)));
        // Nothing more until the break is over
        assert_eq!(pomodoro.tick(start + WORK * 2), None);
        assert_eq!(pomodoro.remaining(start + WORK), None);
    }

    #[test]
    fn every_fourth_break_is_long_then_it_starts_over() {
        let mut now = Instant::now();
        let mut pomodoro = Pomodoro::start(now);
        for round in 1..=ROUNDS {
            assert_eq!(pomodoro.round(), round);
            now += WORK;
            pomodoro.tick(now - REMINDER);
            let expected = if round == ROUNDS { LONG_BREAK } else { SHORT_BREAK };
            assert_eq!(pomodoro.tick(now), Some(Event::BreakDue(expected)));
            now += expected;
            pomodoro.break_over(now);
        }
        assert_eq!(pomodoro.round(), 1);
        assert_eq!(pomodoro.remaining(now), Some(WORK));
    }
}
