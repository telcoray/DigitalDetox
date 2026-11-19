//! "Start on its own each day": on the first use of the PC each day, the Pomodoro starts by
//! itself, so it doesn't depend on remembering it. A card in the corner counts down first,
//! with "Start now" and "Not today".
//!
//! It runs on this PC only: paired devices aren't told, so stopping or pausing it doesn't touch
//! theirs either. Something started on a paired device still takes over, as usual.

use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{Timer, TimerMode};

use crate::reminder::Reminder;
use crate::{App, power, time_format};

/// How often to look whether the PC is being used
const CHECK_EVERY: Duration = Duration::from_secs(10);
/// How long the card waits before the Pomodoro starts
const COUNTDOWN: Duration = Duration::from_secs(30);
/// The PC counts as in use if the mouse or keyboard was touched this recently
const IN_USE: Duration = Duration::from_secs(60);
/// A day starts at 5:00, so working past midnight still belongs to the evening before
const DAY_STARTS_AT_HOUR: i64 = 5;

/// The card, while it's counting down
pub struct Offer {
    card: Reminder,
    starts_at: Instant,
    _timer: Timer,
}

/// Starts looking for the first use of the day. The checks stop when the timer is dropped.
pub fn start_checking() -> Timer {
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, CHECK_EVERY, || crate::with_app(check));
    timer
}

/// "Start on its own each day", in the Pomodoro tab
pub fn toggle(app: &Rc<App>) {
    let mut settings = app.settings.borrow_mut();
    settings.auto_pomodoro = !settings.auto_pomodoro;
    settings.save();
    log!("[auto-pomodoro] turned {}", if settings.auto_pomodoro { "on" } else { "off" });
    app.window.set_auto_pomodoro(settings.auto_pomodoro);
    drop(settings);
    if !app.settings.borrow().auto_pomodoro {
        cancel(app);
    }
}

fn today() -> String {
    day_of(chrono::Local::now().naive_local())
}

fn day_of(time: chrono::NaiveDateTime) -> String {
    (time - chrono::Duration::hours(DAY_STARTS_AT_HOUR)).format("%Y-%m-%d").to_string()
}

fn check(app: &Rc<App>) {
    let today = today();
    {
        let settings = app.settings.borrow();
        if !settings.auto_pomodoro || settings.auto_pomodoro_day == today {
            return;
        }
    }
    // Already started today, by hand or by a paired device: nothing left to offer
    if app.pomodoro.borrow().is_some() {
        return mark_done(app, today);
    }
    if app.session.borrow().is_some() || app.offer.borrow().is_some() {
        return;
    }
    // Someone at the PC, past the Windows lock screen
    if power::idle_time() > IN_USE || !power::desktop_unlocked() {
        return;
    }
    mark_done(app, today);
    offer(app);
}

/// Offered (or running) today: not again until tomorrow, even after a restart
fn mark_done(app: &App, today: String) {
    let mut settings = app.settings.borrow_mut();
    settings.auto_pomodoro_day = today;
    settings.save();
}

fn offer(app: &Rc<App>) {
    log!("[auto-pomodoro] first use of the day, offering a Pomodoro");
    // The buttons act on the next loop iteration: the card is closed from inside its own callback
    let card = Reminder::ask(
        "Pomodoro starting",
        &countdown_text(COUNTDOWN),
        ("Start now", || Timer::single_shot(Duration::ZERO, || crate::with_app(start))),
        ("Not today", || Timer::single_shot(Duration::ZERO, || crate::with_app(cancel))),
    );
    let card = match card {
        Ok(card) => card,
        Err(e) => return log!("[auto-pomodoro] couldn't show the card: {e}"),
    };
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), || crate::with_app(tick));
    *app.offer.borrow_mut() = Some(Offer { card, starts_at: Instant::now() + COUNTDOWN, _timer: timer });
}

fn countdown_text(left: Duration) -> String {
    format!("Starts in {} s, on this PC only.", left.as_secs_f32().ceil() as u64)
}

fn tick(app: &Rc<App>) {
    // Started some other way meanwhile (the tab, the tray, a paired device). Deferred, like
    // `start` below: dropping the offer stops the timer this tick runs from.
    if app.pomodoro.borrow().is_some() || app.session.borrow().is_some() {
        Timer::single_shot(Duration::ZERO, || crate::with_app(|app| drop(app.offer.take())));
        return;
    }
    let left = match app.offer.borrow().as_ref() {
        Some(offer) => offer.starts_at.saturating_duration_since(Instant::now()),
        None => return,
    };
    if left.is_zero() {
        Timer::single_shot(Duration::ZERO, || crate::with_app(start));
        return;
    }
    if let Some(offer) = app.offer.borrow().as_ref() {
        offer.card.set_message(&countdown_text(left));
    }
}

fn start(app: &Rc<App>) {
    if app.offer.take().is_none() || app.pomodoro.borrow().is_some() || app.session.borrow().is_some() {
        return;
    }
    log!("[auto-pomodoro] starting");
    crate::run_pomodoro(app, time_format::now_ms(), None, false);
}

fn cancel(app: &Rc<App>) {
    if app.offer.take().is_some() {
        log!("[auto-pomodoro] not today");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn the_day_starts_at_five() {
        assert_eq!(day_of(at("2026-11-19 09:00")), "2026-11-19");
        assert_eq!(day_of(at("2026-11-19 05:00")), "2026-11-19");
        // Still the evening before
        assert_eq!(day_of(at("2026-11-20 01:30")), "2026-11-19");
    }
}
