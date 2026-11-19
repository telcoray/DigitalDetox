//! The Schedule tab: detoxes at set times on the days chosen, e.g. at 13:00 for 30 minutes from
//! Monday to Friday. A card in the
//! corner counts down the last minute, with "Start now" and "Skip", then the screen locks.
//!
//! They lock this PC only, and are skipped during a Pomodoro, which has breaks of its own.
//! The tab also holds the switch for the break after a long stretch (see break_reminder.rs).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use slint::{Model, ModelRc, SharedString, Timer, TimerMode, VecModel};

use crate::reminder::Reminder;
use crate::session::SessionKind;
use crate::settings::{Appointment, EVERY_DAY};
use crate::{App, ScheduledDetox, power};

const CHECK_EVERY: Duration = Duration::from_secs(5);
/// The card shows up this long before the set time
const WARNING: Duration = Duration::from_secs(60);
/// After sleeping through a set time, the PC doesn't lock when it wakes up
const LATE_BY_AT_MOST: Duration = Duration::from_secs(2 * 60);
/// As many as fit on the tab's first screen
const MAX_APPOINTMENTS: usize = 4;
/// Same limit as a detox typed in the Detox tab
const MAX_MINUTES: u32 = 999;
/// Monday first, as the days are stored
const DAY_NAMES: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const WEEKDAYS: u8 = 0b001_1111;
const WEEKEND: u8 = 0b110_0000;

#[derive(Default)]
pub struct State {
    last_check: Cell<Option<NaiveDateTime>>,
    pending: RefCell<Option<Pending>>,
    // The set time being edited (its place in the list), or None while adding a new one
    editing: Cell<Option<usize>>,
    // The days picked on that screen, Monday first
    new_days: Rc<VecModel<bool>>,
}

impl State {
    pub fn card_shown(&self) -> bool {
        self.pending.borrow().is_some()
    }
}

/// The card, while it counts down to a set time
struct Pending {
    card: Reminder,
    starts_at: Instant,
    minutes: u32,
    _timer: Timer,
}

/// Connects the Schedule tab and starts watching the clock. The checks stop when the timer is
/// dropped.
pub fn setup(app: &Rc<App>) -> Timer {
    let window = &app.window;
    window.on_add_appointment_clicked(|| crate::with_app(start_adding));
    window.on_save_appointment(|time, minutes| crate::with_app(|app| save(app, &time, &minutes)));
    window.on_cancel_appointment(|| {
        crate::with_app(|app| {
            app.window.set_adding_appointment(false);
            update_view(app);
        })
    });
    window.on_toggle_day(|index| {
        crate::with_app(|app| {
            let days = &app.schedule.new_days;
            if let Some(on) = days.row_data(index as usize) {
                days.set_row_data(index as usize, !on);
            }
        })
    });
    window.set_new_days(ModelRc::from(app.schedule.new_days.clone()));
    window.on_edit_appointment(|index| crate::with_app(|app| start_editing(app, index as usize)));
    window.on_remove_appointment(|index| crate::with_app(|app| remove(app, index as usize)));
    window.on_break_reminder_toggled(|| crate::with_app(crate::break_reminder::toggle));
    window.set_break_reminder(app.settings.borrow().break_reminder);
    update_view(app);

    let timer = Timer::default();
    timer.start(TimerMode::Repeated, CHECK_EVERY, || crate::with_app(check));
    timer
}

fn start_adding(app: &Rc<App>) {
    if app.settings.borrow().schedule.len() >= MAX_APPOINTMENTS {
        return app.window.set_schedule_status(format!("Up to {MAX_APPOINTMENTS} set times").into());
    }
    app.schedule.editing.set(None);
    let window = &app.window;
    window.set_new_time(SharedString::new());
    window.set_new_minutes("30".into());
    set_new_days(app, EVERY_DAY);
    window.set_schedule_status(SharedString::new());
    window.set_adding_appointment(true);
}

/// The same screen as adding one, filled in with what it was
fn start_editing(app: &Rc<App>, index: usize) {
    let Some(appointment) = app.settings.borrow().schedule.get(index).copied() else { return };
    app.schedule.editing.set(Some(index));
    let window = &app.window;
    window.set_new_time(time_text(appointment.hour, appointment.minute).into());
    window.set_new_minutes(appointment.minutes.to_string().into());
    set_new_days(app, appointment.days);
    window.set_schedule_status(SharedString::new());
    window.set_adding_appointment(true);
}

fn set_new_days(app: &App, days: u8) {
    let picked: Vec<bool> = (0..7).map(|day| days & (1 << day) != 0).collect();
    app.schedule.new_days.set_vec(picked);
}

fn save(app: &Rc<App>, time: &str, minutes: &str) {
    let window = &app.window;
    let Some((hour, minute)) = parse_time(time) else {
        return window.set_schedule_status("Enter a time like 13:00".into());
    };
    let Some(minutes) = minutes.trim().parse::<u32>().ok().filter(|m| (1..=MAX_MINUTES).contains(m)) else {
        return window.set_schedule_status("Enter a number of minutes (1 or more)".into());
    };
    let days = (0..7).filter(|&day| app.schedule.new_days.row_data(day) == Some(true)).fold(0u8, |days, day| days | 1 << day);
    if days == 0 {
        return window.set_schedule_status("Pick at least one day".into());
    }
    let appointment = Appointment { hour, minute, minutes, days };
    let editing = app.schedule.editing.get();
    let mut settings = app.settings.borrow_mut();
    // The one being edited may keep its time
    let taken = settings.schedule.iter().enumerate().any(|(i, a)| Some(i) != editing && (a.hour, a.minute) == (hour, minute));
    if taken {
        return window.set_schedule_status(format!("There's already one at {}", time_text(hour, minute)).into());
    }
    match editing.filter(|&i| i < settings.schedule.len()) {
        Some(index) => {
            let old = std::mem::replace(&mut settings.schedule[index], appointment);
            log!("[schedule] changed {} to {} for {minutes} min, {}", time_text(old.hour, old.minute), time_text(hour, minute), days_text(days));
        }
        None => {
            settings.schedule.push(appointment);
            log!("[schedule] added {} for {minutes} min, {}", time_text(hour, minute), days_text(days));
        }
    }
    settings.schedule.sort();
    settings.save();
    drop(settings);
    app.schedule.editing.set(None);
    window.set_adding_appointment(false);
    update_view(app);
}

fn remove(app: &Rc<App>, index: usize) {
    let mut settings = app.settings.borrow_mut();
    if index < settings.schedule.len() {
        let removed = settings.schedule.remove(index);
        settings.save();
        log!("[schedule] removed {}", time_text(removed.hour, removed.minute));
    }
    drop(settings);
    update_view(app);
}

/// "13:00", "13.00", "1300", "9:5" and "9" are all accepted
fn parse_time(text: &str) -> Option<(u32, u32)> {
    let text = text.trim();
    let (hour, minute) = match text.split_once([':', '.']) {
        Some((hour, minute)) => (hour, minute),
        None if text.len() > 2 => text.split_at(text.len() - 2),
        None => (text, "0"),
    };
    let (hour, minute) = (hour.trim().parse::<u32>().ok()?, minute.trim().parse::<u32>().ok()?);
    (hour < 24 && minute < 60).then_some((hour, minute))
}

fn time_text(hour: u32, minute: u32) -> String {
    format!("{hour:02}:{minute:02}")
}

/// "Every day", "Weekdays", "Weekends", or the days themselves: "Mon Wed Fri"
fn days_text(days: u8) -> String {
    match days {
        EVERY_DAY => "Every day".into(),
        WEEKDAYS => "Weekdays".into(),
        WEEKEND => "Weekends".into(),
        _ => (0..7).filter(|day| days & (1 << day) != 0).map(|day| &DAY_NAMES[day][..3]).collect::<Vec<_>>().join(" "),
    }
}

/// Whether a set time with these days happens on `date`
fn happens_on(days: u8, date: NaiveDate) -> bool {
    days & (1 << date.weekday().num_days_from_monday()) != 0
}

/// Refreshes the list of set times, and the line under it
pub fn update_view(app: &App) {
    let settings = app.settings.borrow();
    let items: Vec<ScheduledDetox> = settings
        .schedule
        .iter()
        .map(|a| ScheduledDetox {
            time: time_text(a.hour, a.minute).into(),
            days: days_text(a.days).into(),
            minutes: a.minutes as i32,
        })
        .collect();
    app.window.set_appointments(ModelRc::new(VecModel::from(items)));
    // While a time is being typed, the line shows what's wrong with it instead
    if app.window.get_adding_appointment() {
        return;
    }
    let now = chrono::Local::now().naive_local();
    let status = match next(&settings.schedule, now) {
        Some(at) => {
            let day = match (at.date() - now.date()).num_days() {
                0 => "today".to_string(),
                1 => "tomorrow".to_string(),
                _ => format!("on {}", DAY_NAMES[at.weekday().num_days_from_monday() as usize]),
            };
            format!("Next one {day} at {}", time_text(at.hour(), at.minute()))
        }
        None => String::new(),
    };
    app.window.set_schedule_status(status.into());
}

/// When the next set time after `now` is, within the coming week
fn next(schedule: &[Appointment], now: NaiveDateTime) -> Option<NaiveDateTime> {
    now.date()
        .iter_days()
        .take(8)
        .flat_map(|date| {
            schedule
                .iter()
                .filter(move |a| happens_on(a.days, date))
                .filter_map(move |a| date.and_hms_opt(a.hour, a.minute, 0))
        })
        .filter(|&at| at > now)
        .min()
}

/// The set time whose card is due: its warning time came between the last check and now
fn due(schedule: &[Appointment], last: NaiveDateTime, now: NaiveDateTime) -> Option<Appointment> {
    let days = [last.date(), now.date(), now.date().succ_opt()?];
    let warning = chrono::Duration::from_std(WARNING).ok()?;
    let late_by_at_most = chrono::Duration::from_std(LATE_BY_AT_MOST).ok()?;
    schedule.iter().copied().find(|a| {
        days.iter().any(|day| {
            let Some(at) = day.and_hms_opt(a.hour, a.minute, 0).filter(|at| happens_on(a.days, at.date())) else { return false };
            let warn_at = at - warning;
            last < warn_at && warn_at <= now && now - warn_at <= late_by_at_most
        })
    })
}

fn check(app: &Rc<App>) {
    let state = &app.schedule;
    let now = chrono::Local::now().naive_local();
    // A new day may have started: "Next one tomorrow" becomes "today"
    if state.last_check.get().is_some_and(|last| last.date() != now.date() || last.minute() != now.minute()) {
        update_view(app);
    }
    let Some(last) = state.last_check.replace(Some(now)) else { return };
    let Some(appointment) = due(&app.settings.borrow().schedule, last, now) else { return };
    let at = time_text(appointment.hour, appointment.minute);
    if app.session.borrow().is_some() || state.pending.borrow().is_some() {
        return log!("[schedule] {at}: something else is on the screen, skipped");
    }
    if app.pomodoro.borrow().is_some() {
        return log!("[schedule] {at}: a Pomodoro is running, skipped");
    }
    if !power::desktop_unlocked() {
        return log!("[schedule] {at}: the PC is locked, skipped");
    }
    offer(app, appointment);
}

fn offer(app: &Rc<App>, appointment: Appointment) {
    let at = time_text(appointment.hour, appointment.minute);
    log!("[schedule] {at}, {} min: one minute to go", appointment.minutes);
    // Its own card takes the corner
    app.break_reminder.hide_card();
    crate::auto_pomodoro::cancel(app);
    // The buttons act on the next loop iteration: the card is closed from inside its own callback
    let card = Reminder::ask(
        &format!("Detox at {at}"),
        &countdown_text(WARNING, appointment.minutes),
        ("Start now", || Timer::single_shot(Duration::ZERO, || crate::with_app(start))),
        ("Skip", || Timer::single_shot(Duration::ZERO, || crate::with_app(skip))),
    );
    let card = match card {
        Ok(card) => card,
        Err(e) => return log!("[schedule] couldn't show the card: {e}"),
    };
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), || crate::with_app(tick));
    let pending = Pending { card, starts_at: Instant::now() + WARNING, minutes: appointment.minutes, _timer: timer };
    *app.schedule.pending.borrow_mut() = Some(pending);
}

fn countdown_text(left: Duration, minutes: u32) -> String {
    format!("Locks in {} s, for {minutes} minutes.", left.as_secs_f32().ceil() as u64)
}

fn tick(app: &Rc<App>) {
    // Something else locked the screen or started a Pomodoro meanwhile. Deferred, like `start`
    // below: dropping the card stops the timer this tick runs from.
    if app.session.borrow().is_some() || app.pomodoro.borrow().is_some() {
        Timer::single_shot(Duration::ZERO, || crate::with_app(|app| drop(app.schedule.pending.take())));
        return;
    }
    let (left, minutes) = match app.schedule.pending.borrow().as_ref() {
        Some(pending) => (pending.starts_at.saturating_duration_since(Instant::now()), pending.minutes),
        None => return,
    };
    if left.is_zero() {
        Timer::single_shot(Duration::ZERO, || crate::with_app(start));
        return;
    }
    if let Some(pending) = app.schedule.pending.borrow().as_ref() {
        pending.card.set_message(&countdown_text(left, minutes));
    }
}

fn start(app: &Rc<App>) {
    let Some(pending) = app.schedule.pending.take() else { return };
    let minutes = pending.minutes;
    drop(pending);
    if app.session.borrow().is_some() || app.pomodoro.borrow().is_some() {
        return;
    }
    log!("[schedule] detox of {minutes} min");
    crate::start_lock(app, Duration::from_secs(minutes as u64 * 60), SessionKind::Detox);
}

fn skip(app: &Rc<App>) {
    if app.schedule.pending.take().is_some() {
        log!("[schedule] skipped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn appointment(hour: u32, minute: u32) -> Appointment {
        Appointment { hour, minute, minutes: 30, days: EVERY_DAY }
    }

    #[test]
    fn times_are_read_in_several_ways() {
        assert_eq!(parse_time("13:00"), Some((13, 0)));
        assert_eq!(parse_time(" 13.05 "), Some((13, 5)));
        assert_eq!(parse_time("1330"), Some((13, 30)));
        assert_eq!(parse_time("930"), Some((9, 30)));
        assert_eq!(parse_time("9"), Some((9, 0)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("12:60"), None);
        assert_eq!(parse_time(""), None);
        assert_eq!(parse_time("noon"), None);
    }

    #[test]
    fn the_card_is_due_a_minute_before() {
        let schedule = [appointment(13, 0)];
        assert_eq!(due(&schedule, at("2026-11-19 12:58:55"), at("2026-11-19 12:59:00")), Some(schedule[0]));
        assert_eq!(due(&schedule, at("2026-11-19 12:59:00"), at("2026-11-19 12:59:05")), None);
        assert_eq!(due(&schedule, at("2026-11-19 12:58:50"), at("2026-11-19 12:58:55")), None);
    }

    #[test]
    fn midnight_is_due_the_evening_before() {
        let schedule = [appointment(0, 0)];
        assert_eq!(due(&schedule, at("2026-11-19 23:58:58"), at("2026-11-19 23:59:03")), Some(schedule[0]));
    }

    #[test]
    fn a_time_slept_through_is_not_due() {
        let schedule = [appointment(13, 0)];
        assert_eq!(due(&schedule, at("2026-11-19 11:00:00"), at("2026-11-19 15:00:00")), None);
        // Woken up just after: still worth it
        assert_eq!(due(&schedule, at("2026-11-19 12:00:00"), at("2026-11-19 13:00:30")), Some(schedule[0]));
    }

    #[test]
    fn the_next_one_may_be_tomorrow() {
        let schedule = [appointment(13, 0), appointment(22, 30)];
        assert_eq!(next(&schedule, at("2026-11-19 09:00:00")), Some(at("2026-11-19 13:00:00")));
        assert_eq!(next(&schedule, at("2026-11-19 14:00:00")), Some(at("2026-11-19 22:30:00")));
        assert_eq!(next(&schedule, at("2026-11-19 23:00:00")), Some(at("2026-11-20 13:00:00")));
        assert_eq!(next(&[], at("2026-11-19 23:00:00")), None);
    }

    #[test]
    fn only_on_the_days_chosen() {
        // 2026-11-19 is a Thursday
        let weekdays = [Appointment { days: WEEKDAYS, ..appointment(13, 0) }];
        assert_eq!(due(&weekdays, at("2026-11-19 12:58:55"), at("2026-11-19 12:59:00")), Some(weekdays[0]));
        assert_eq!(due(&weekdays, at("2026-11-21 12:58:55"), at("2026-11-21 12:59:00")), None);
        // Friday after it's gone: the next one is on Monday
        assert_eq!(next(&weekdays, at("2026-11-20 14:00:00")), Some(at("2026-11-23 13:00:00")));
        // A Monday at midnight: its card comes on Sunday evening
        let monday = [Appointment { days: 1, ..appointment(0, 0) }];
        assert_eq!(due(&monday, at("2026-11-22 23:58:58"), at("2026-11-22 23:59:03")), Some(monday[0]));
        assert_eq!(due(&monday, at("2026-11-19 23:58:58"), at("2026-11-19 23:59:03")), None);
    }

    #[test]
    fn days_read_naturally() {
        assert_eq!(days_text(EVERY_DAY), "Every day");
        assert_eq!(days_text(WEEKDAYS), "Weekdays");
        assert_eq!(days_text(WEEKEND), "Weekends");
        assert_eq!(days_text(0b001_0101), "Mon Wed Fri");
    }
}
