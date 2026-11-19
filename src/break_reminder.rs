//! "Break after 90 minutes": after a long stretch at the PC without a pause, a card in the
//! corner offers a 10-minute break. Nothing happens unless it's accepted; "Later" asks again
//! a quarter of an hour later. Stepping away for a few minutes counts as a pause.
//!
//! Never during a Pomodoro, which has breaks of its own. The break locks this PC only.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use slint::{Timer, TimerMode};

use crate::reminder::Reminder;
use crate::session::SessionKind;
use crate::{App, power};

/// How often to look whether the PC is being used
const CHECK_EVERY: Duration = Duration::from_secs(10);
/// How long at the PC before a break is offered
pub const AT_THE_PC_FOR: Duration = Duration::from_secs(90 * 60);
/// This long without touching the mouse or keyboard counts as a pause
const PAUSE: Duration = Duration::from_secs(5 * 60);
/// A longer gap between two checks means the PC was asleep: a pause too
const SLEPT: Duration = Duration::from_secs(60);
pub const BREAK: Duration = Duration::from_secs(10 * 60);
/// "Later" asks again after this
const LATER: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct State {
    // Wall-clock time, so a PC that slept in between doesn't count as used
    last_check: Cell<Option<SystemTime>>,
    at_the_pc_since: Cell<Option<SystemTime>>,
    not_before: Cell<Option<SystemTime>>,
    card: RefCell<Option<Reminder>>,
}

impl State {
    pub fn card_shown(&self) -> bool {
        self.card.borrow().is_some()
    }

    /// Makes room for another card. The time at the PC still counts.
    pub fn hide_card(&self) {
        let card = self.card.take();
        drop(card);
    }

    /// Back to zero, as after a pause
    fn reset(&self) {
        self.at_the_pc_since.set(None);
        self.not_before.set(None);
        // Taken out first: dropping the card must not happen inside the borrow
        let card = self.card.take();
        drop(card);
    }
}

/// Starts watching how long the PC is used. The checks stop when the timer is dropped.
pub fn start_checking() -> Timer {
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, CHECK_EVERY, || crate::with_app(check));
    timer
}

/// "Break after 90 minutes", in the Schedule tab
pub fn toggle(app: &Rc<App>) {
    let mut settings = app.settings.borrow_mut();
    settings.break_reminder = !settings.break_reminder;
    settings.save();
    let on = settings.break_reminder;
    drop(settings);
    log!("[break-reminder] turned {}", if on { "on" } else { "off" });
    app.window.set_break_reminder(on);
    app.break_reminder.reset();
}

fn check(app: &Rc<App>) {
    let state = &app.break_reminder;
    let now = SystemTime::now();
    let since_last_check = state.last_check.replace(Some(now)).and_then(|last| now.duration_since(last).ok());
    let pause = since_last_check.is_none_or(|gap| gap > SLEPT)
        || power::idle_time() >= PAUSE
        || !power::desktop_unlocked()
        || app.session.borrow().is_some()
        || app.pomodoro.borrow().is_some();
    if pause || !app.settings.borrow().break_reminder {
        if state.at_the_pc_since.get().is_some() {
            state.reset();
        }
        return;
    }
    let since = state.at_the_pc_since.get().unwrap_or(now);
    state.at_the_pc_since.set(Some(since));
    let used = now.duration_since(since).unwrap_or_default();
    let later = state.not_before.get().is_some_and(|not_before| now < not_before);
    if used < AT_THE_PC_FOR || later || crate::card_shown(app) {
        return;
    }
    offer(app, used);
}

fn offer(app: &Rc<App>, used: Duration) {
    log!("[break-reminder] at the PC for {} min, offering a break", used.as_secs() / 60);
    // The buttons act on the next loop iteration: the card is closed from inside its own callback
    let card = Reminder::ask(
        &format!("{} at the PC", hours_and_minutes(used)),
        &format!("Time for a {}-minute break?", BREAK.as_secs() / 60),
        ("Take a break", || Timer::single_shot(Duration::ZERO, || crate::with_app(take_break))),
        ("Later", || Timer::single_shot(Duration::ZERO, || crate::with_app(later))),
    );
    match card {
        Ok(card) => *app.break_reminder.card.borrow_mut() = Some(card),
        Err(e) => log!("[break-reminder] couldn't show the card: {e}"),
    }
}

/// "1 h 30 min", "2 h"
fn hours_and_minutes(time: Duration) -> String {
    let minutes = time.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

fn take_break(app: &Rc<App>) {
    let state = &app.break_reminder;
    if state.card.take().is_none() || app.session.borrow().is_some() || app.pomodoro.borrow().is_some() {
        return;
    }
    log!("[break-reminder] break");
    state.reset();
    crate::start_lock(app, BREAK, SessionKind::Break);
}

fn later(app: &Rc<App>) {
    let state = &app.break_reminder;
    if state.card.take().is_some() {
        log!("[break-reminder] later");
        state.not_before.set(Some(SystemTime::now() + LATER));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_at_the_pc_reads_naturally() {
        assert_eq!(hours_and_minutes(Duration::from_secs(45 * 60)), "45 min");
        assert_eq!(hours_and_minutes(Duration::from_secs(90 * 60 + 59)), "1 h 30 min");
        assert_eq!(hours_and_minutes(Duration::from_secs(120 * 60)), "2 h");
    }
}
