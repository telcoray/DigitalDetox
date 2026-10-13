//! The gradual "switching off" of the screen during a detox:
//!
//! awake ──(1 min without input)──▶ black ──(2 more min)──▶ monitor off
//!   ▲                                 │                        │
//!   └───────── mouse or keyboard ─────┴────────────────────────┘
//!
//! After being woken up the timer stays visible for 10 seconds, then fades to black again.
//! The "Turn off screen" button jumps straight to black, with the monitor off shortly after.
//!
//! On PCs with Modern Standby (most recent laptops) the monitor is never switched off: there
//! Windows would treat it like the power button, go to standby and ask to sign in again.
//! A black screen with the brightness at the minimum (see brightness.rs) is just as dark.

use std::time::{Duration, Instant};

use crate::power;

/// Software dimming while the timer is visible. Set to 0.0 to turn it off.
pub const DIM_LEVEL: f32 = 0.45;
const BLACK_LEVEL: f32 = 1.0;

/// Idle time before the screen fades to black, at the start of the detox
const FIRST_BLACK_AFTER: Duration = Duration::from_secs(60);
/// Idle time before it fades to black again after being woken up
const WAKE_BLACK_AFTER: Duration = Duration::from_secs(10);
/// How long the screen stays black before the monitor is switched off
const IDLE_OFF_AFTER: Duration = Duration::from_secs(120);
/// Same, after the "Turn off screen" button
const BUTTON_OFF_AFTER: Duration = Duration::from_secs(2);
/// Input right after a change is ignored: the hand still on the mouse after clicking the button,
/// or the spurious input some systems generate when the monitor is switched off
const GRACE: Duration = Duration::from_secs(2);

/// Speed of the transitions
pub const DIM_FADE: Duration = Duration::from_secs(3);
const BLACK_FADE: Duration = Duration::from_secs(2);
const WAKE_FADE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Awake { black_after: Duration },
    Black { off_after: Duration },
    Off,
}

pub struct ScreenControl {
    state: State,
    /// When the current state began
    since: Instant,
    /// False with Modern Standby: the screen then stays black instead of being switched off
    can_power_off: bool,
}

/// What the windows should show after an update: shade level and how fast to get there
pub struct Shade {
    pub level: f32,
    pub fade: Duration,
}

impl ScreenControl {
    pub fn new() -> Self {
        let can_power_off = !power::has_modern_standby();
        log!("[screen] monitor power-off {}", if can_power_off { "enabled" } else { "disabled (Modern Standby)" });
        Self { state: State::Awake { black_after: FIRST_BLACK_AFTER }, since: Instant::now(), can_power_off }
    }

    /// Called on every timer tick. Returns the new shade when it has to change.
    /// `window` is needed to switch the monitor off.
    pub fn tick(&mut self, window: &slint::Window) -> Option<Shade> {
        let idle = power::idle_time();
        let in_state = self.since.elapsed();
        // Has there been any input since we entered the current state (after the grace period)?
        let woken = in_state > GRACE && idle < in_state;

        match self.state {
            State::Awake { black_after } if idle >= black_after => Some(self.go_black(IDLE_OFF_AFTER)),
            State::Black { .. } | State::Off if woken => {
                log!("[screen] input detected (last input {idle:.1?} ago, {in_state:.0?} into {:?})", self.state);
                Some(self.wake())
            }
            State::Black { off_after } if self.can_power_off && in_state >= off_after => {
                power::turn_monitors_off(window);
                self.enter(State::Off);
                None
            }
            _ => None,
        }
    }

    /// The "Turn off screen" button
    pub fn turn_off_now(&mut self) -> Shade {
        self.go_black(BUTTON_OFF_AFTER)
    }

    fn go_black(&mut self, off_after: Duration) -> Shade {
        self.enter(State::Black { off_after });
        Shade { level: BLACK_LEVEL, fade: BLACK_FADE }
    }

    fn wake(&mut self) -> Shade {
        self.enter(State::Awake { black_after: WAKE_BLACK_AFTER });
        Shade { level: DIM_LEVEL, fade: WAKE_FADE }
    }

    fn enter(&mut self, state: State) {
        // Visible in the console of `cargo run`, handy to check the timings
        log!("[screen] {:?} -> {:?} after {:.0?}", self.state, state, self.since.elapsed());
        self.state = state;
        self.since = Instant::now();
    }
}
