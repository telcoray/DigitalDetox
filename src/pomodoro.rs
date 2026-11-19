//! Pomodoro mode: rounds of work with the PC free, each followed by a break with the screen
//! locked; after the last round of a cycle the break is a long one. Usually 25 minutes of work,
//! 5-minute breaks and a 15-minute break every 4th round, but the `Plan` can change all of
//! them. It goes on until it's stopped, or after a set number of rounds.
//!
//! The whole schedule follows from the moment the Pomodoro started (a Unix time, in
//! milliseconds) and its plan. Two paired devices that know both are therefore always in the
//! same round and lock at the same moment, even if one of them joins halfway or wakes up from
//! sleep. A pause freezes the schedule; resuming moves the start later by the pause's length.
//! main.rs ticks a `Tracker` every second, shows the reminder and starts the breaks.

use std::time::Duration;

pub use crate::sync::Plan;

/// How long before a break the reminder appears
pub const REMINDER: Duration = Duration::from_secs(60);
/// A break that has less than this left (e.g. when joining at its very end) isn't worth locking for
const MIN_BREAK: Duration = Duration::from_secs(5);

fn minutes(minutes: u32) -> Duration {
    Duration::from_secs(minutes as u64 * 60)
}

impl Plan {
    pub fn work_length(&self) -> Duration {
        minutes(self.work)
    }

    /// The break that follows work round `round` of a cycle
    pub fn break_length(&self, round: u32) -> Duration {
        minutes(if round == self.rounds { self.long_break } else { self.short_break })
    }

    fn cycle_length(&self) -> Duration {
        (1..=self.rounds).map(|round| self.work_length() + self.break_length(round)).sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// `round`: within the cycle, from 1. `number`: counted from the start, from 1.
    Work { round: u32, number: u32, remaining: Duration },
    Break { round: u32, number: u32, remaining: Duration },
    /// All the rounds of a Pomodoro with a set number of them are over
    Done,
}

impl Phase {
    /// Within the cycle, from 1
    pub fn round(&self) -> u32 {
        match *self {
            Phase::Work { round, .. } | Phase::Break { round, .. } => round,
            Phase::Done => 1,
        }
    }

    /// Counted from the start, from 1
    pub fn number(&self) -> u32 {
        match *self {
            Phase::Work { number, .. } | Phase::Break { number, .. } => number,
            Phase::Done => 1,
        }
    }
}

/// Where a Pomodoro started at `start_ms` is at `now_ms`
pub fn phase_at(plan: &Plan, start_ms: u64, now_ms: u64) -> Phase {
    let plan = plan.sane();
    let mut t = Duration::from_millis(now_ms.saturating_sub(start_ms));
    // Until stopped: the same cycle over and over
    let (cycles_done, rounds) = if plan.total == 0 {
        let cycle = plan.cycle_length().as_millis() as u64;
        let elapsed = t.as_millis() as u64;
        t = Duration::from_millis(elapsed % cycle);
        ((elapsed / cycle) as u32, plan.rounds)
    } else {
        (0, plan.total)
    };
    for count in 1..=rounds {
        let round = (count - 1) % plan.rounds + 1;
        let number = cycles_done * plan.rounds + count;
        if t < plan.work_length() {
            return Phase::Work { round, number, remaining: plan.work_length() - t };
        }
        t -= plan.work_length();
        let length = plan.break_length(round);
        if t < length {
            return Phase::Break { round, number, remaining: length - t };
        }
        t -= length;
    }
    if plan.total == 0 {
        // Only reached by rounding at the very end of a cycle
        Phase::Work { round: 1, number: (cycles_done + 1) * plan.rounds + 1, remaining: plan.work_length() }
    } else {
        Phase::Done
    }
}

/// What a tick asks main.rs to do
#[derive(Debug, PartialEq)]
pub enum Event {
    /// A minute left: time to save your work
    Remind,
    /// A break has begun: lock the screen for this long
    BreakDue(Duration),
    /// The last round and its break are over
    Finished,
}

/// Follows a running Pomodoro and reports when something has to happen
pub struct Tracker {
    start_ms: u64,
    // When it was paused, while it is
    paused_at: Option<u64>,
    plan: Plan,
    // The phase seen at the last tick, as (round number, is a break)
    last: Option<(u32, bool)>,
    reminded: bool,
}

impl Tracker {
    pub fn new(start_ms: u64, paused_at: Option<u64>, plan: Plan) -> Self {
        Self { start_ms, paused_at, plan: plan.sane(), last: None, reminded: false }
    }

    pub fn start_ms(&self) -> u64 {
        self.start_ms
    }

    pub fn paused_at(&self) -> Option<u64> {
        self.paused_at
    }

    pub fn plan(&self) -> Plan {
        self.plan
    }

    /// While paused, the phase stays where it was when the pause began
    pub fn phase(&self, now_ms: u64) -> Phase {
        phase_at(&self.plan, self.start_ms, self.paused_at.unwrap_or(now_ms))
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
        let key = (phase.number(), matches!(phase, Phase::Break { .. }));
        let entered = self.last.replace(key) != Some(key);
        match phase {
            Phase::Done => Some(Event::Finished),
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

    const WORK: Duration = Duration::from_secs(25 * 60);
    const SHORT_BREAK: Duration = Duration::from_secs(5 * 60);
    const LONG_BREAK: Duration = Duration::from_secs(15 * 60);

    fn usual() -> Plan {
        Plan::default()
    }

    #[test]
    fn follows_the_schedule() {
        let start = 1_000_000;
        let plan = usual();
        assert_eq!(phase_at(&plan, start, start), Phase::Work { round: 1, number: 1, remaining: WORK });
        assert_eq!(phase_at(&plan, start, start + ms(WORK)), Phase::Break { round: 1, number: 1, remaining: SHORT_BREAK });
        let fourth_break = start + ms(WORK * 4 + SHORT_BREAK * 3);
        assert_eq!(phase_at(&plan, start, fourth_break), Phase::Break { round: 4, number: 4, remaining: LONG_BREAK });
        // Back to round 1 after the long break, as the 5th in all
        assert_eq!(phase_at(&plan, start, fourth_break + ms(LONG_BREAK)), Phase::Work { round: 1, number: 5, remaining: WORK });
    }

    #[test]
    fn follows_a_plan_of_its_own() {
        let plan = Plan { work: 50, short_break: 10, long_break: 30, rounds: 2, total: 0 };
        let min = |m: u64| m * 60_000;
        assert_eq!(phase_at(&plan, 0, min(50)), Phase::Break { round: 1, number: 1, remaining: Duration::from_secs(600) });
        assert_eq!(phase_at(&plan, 0, min(110)), Phase::Break { round: 2, number: 2, remaining: Duration::from_secs(1800) });
        assert_eq!(phase_at(&plan, 0, min(140)), Phase::Work { round: 1, number: 3, remaining: Duration::from_secs(3000) });
    }

    #[test]
    fn stops_after_the_rounds_set() {
        // 3 rounds, with a long break every 2nd: work, short, work, long, work, short, done
        let plan = Plan { work: 10, short_break: 2, long_break: 5, rounds: 2, total: 3 };
        let min = |m: u64| m * 60_000;
        assert_eq!(phase_at(&plan, 0, min(12) + min(10)), Phase::Break { round: 2, number: 2, remaining: Duration::from_secs(300) });
        assert_eq!(phase_at(&plan, 0, min(27)), Phase::Work { round: 1, number: 3, remaining: Duration::from_secs(600) });
        assert_eq!(phase_at(&plan, 0, min(37)), Phase::Break { round: 1, number: 3, remaining: Duration::from_secs(120) });
        assert_eq!(phase_at(&plan, 0, min(39)), Phase::Done);
        let mut tracker = Tracker::new(0, None, plan);
        assert_eq!(tracker.tick(min(39)), Some(Event::Finished));
    }

    #[test]
    fn a_broken_plan_is_made_sane() {
        let plan = Plan { work: 0, short_break: 0, long_break: 0, rounds: 0, total: 0 };
        assert_eq!(phase_at(&plan, 0, 0), Phase::Work { round: 1, number: 1, remaining: Duration::from_secs(60) });
    }

    #[test]
    fn reminds_once_then_locks_once() {
        let start = 0;
        let mut tracker = Tracker::new(start, None, usual());
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
        let mut tracker = Tracker::new(0, None, usual());
        let now = ms(WORK) + 60_000;
        assert_eq!(tracker.tick(now), Some(Event::BreakDue(SHORT_BREAK - Duration::from_secs(60))));
        // Too late to bother
        let mut tracker = Tracker::new(0, None, usual());
        assert_eq!(tracker.tick(ms(WORK + SHORT_BREAK) - 2000), None);
    }

    #[test]
    fn a_pause_freezes_the_countdown() {
        let mut tracker = Tracker::new(0, None, usual());
        tracker.tick(0);
        tracker.pause(ms(WORK) - 300_000);
        // Long past the end of the work round, but paused: no break
        assert_eq!(tracker.tick(ms(WORK) * 3), None);
        assert_eq!(tracker.phase(ms(WORK) * 3), Phase::Work { round: 1, number: 1, remaining: Duration::from_secs(300) });
        tracker.resume(ms(WORK) * 3);
        assert_eq!(tracker.phase(ms(WORK) * 3), Phase::Work { round: 1, number: 1, remaining: Duration::from_secs(300) });
        assert_eq!(tracker.tick(ms(WORK) * 3 + 300_000), Some(Event::BreakDue(SHORT_BREAK)));
    }
}
