// In release builds, don't open a console window next to the app
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use slint::{CloseRequestResponse, ComponentHandle, SharedString, Timer, TimerMode};

/// Diagnostic message, to the console and to the log file (see log.rs).
/// Defined before the modules so they can all use it.
macro_rules! log {
    ($($arg:tt)*) => { $crate::log::write(&format!($($arg)*)) };
}

mod autostart;
mod brightness;
mod foreground;
mod keyboard_hook;
mod log;
mod pomodoro;
mod power;
mod reminder;
mod screen;
mod session;
mod single_instance;
mod taskbar;
mod time_format;
mod tray;

use pomodoro::Pomodoro;
use reminder::Reminder;
use session::{LockSession, SessionKind};

// Brings in the Rust types generated from ui/app.slint (`MainWindow`, `LockWindow`, `CoverWindow`)
slint::include_modules!();

/// At most 3 digits (so up to 999 minutes)
const MAX_DIGITS: usize = 3;

/// Requests coming from outside the main window: the tray icon and its menu, or a second copy
/// of the app being started
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Open,
    Start(u64),
    TogglePomodoro,
    ToggleAutostart,
    Quit,
}

/// Everything the app keeps while it runs, mostly hidden in the notification area
struct App {
    window: MainWindow,
    // The running detox, if any. Dropping it closes the lock screen.
    session: RefCell<Option<LockSession>>,
    // The Pomodoro mode, while it's on
    pomodoro: RefCell<Option<PomodoroRun>>,
    tray: Option<tray::Tray>,
    _preview_timer: Timer,
}

struct PomodoroRun {
    cycle: Pomodoro,
    // Ticks the cycle every second
    _timer: Timer,
    // The "Break in 1 minute" card, while it's shown
    reminder: Option<Reminder>,
}

thread_local! {
    // The app lives on the UI thread; actions reach it through here
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Runs `action` on the UI thread. Can be called from any thread.
pub fn dispatch(action: Action) {
    let _ = slint::invoke_from_event_loop(move || with_app(|app| handle(app, action)));
}

fn with_app(f: impl FnOnce(&Rc<App>)) {
    // Clone the Rc out first, so `f` is free to use APP again
    if let Some(app) = APP.with(|slot| slot.borrow().clone()) {
        f(&app);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // Held until main returns. A second copy just asks this one to show its window, then exits.
    let instance = match single_instance::SingleInstance::acquire() {
        Ok(instance) => instance,
        Err(single_instance::AlreadyRunning) => {
            single_instance::show_running_instance();
            return Ok(());
        }
    };

    log::start();
    // A release build has no console, so a panic would leave no trace; and the app may not get
    // to restore the taskbars if it goes down (a panic inside a Windows callback aborts it)
    std::panic::set_hook(Box::new(|info| {
        log!("[panic] {info}");
        taskbar::recover();
    }));

    // If a previous run was closed abruptly during a detox, give the screen its brightness and
    // the taskbars back
    brightness::recover();
    taskbar::recover();

    // Started with Windows: stay in the system tray until the user opens the window
    let background = std::env::args().any(|arg| arg == autostart::BACKGROUND_FLAG);

    let window = MainWindow::new()?;
    let tray = tray::Tray::new(autostart::is_enabled())
        .inspect_err(|e| log!("Could not create the tray icon: {e}"))
        .ok();
    setup_main_window(&window, tray.is_some());

    // The clock moves on while the window is open: keep "Unlocks at …" and "First break at …" current
    let preview_timer = Timer::default();
    preview_timer.start(TimerMode::Repeated, Duration::from_secs(5), || {
        with_app(|app| {
            update_unlock_preview(&app.window);
            update_pomodoro_view(app);
        })
    });

    let app = Rc::new(App {
        window,
        session: RefCell::default(),
        pomodoro: RefCell::default(),
        tray,
        _preview_timer: preview_timer,
    });
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    update_pomodoro_view(&app);
    instance.listen_for_show_requests(|| dispatch(Action::Open));

    // Without a tray icon there would be no way to reach a hidden window, so then it opens anyway
    if background && app.tray.is_some() {
        log!("[app] started in the background");
    } else {
        show_window(&app);
    }
    drop(app);
    // Keeps running with every window hidden, until Quit is chosen from the tray menu
    slint::run_event_loop_until_quit()?;

    // Drop the app now (this removes the tray icon), while the UI thread is still alive
    APP.with(|slot| slot.borrow_mut().take());
    Ok(())
}

fn handle(app: &Rc<App>, action: Action) {
    log!("[app] action {action:?} (window visible: {})", app.window.window().is_visible());
    // Nothing from outside may interfere with a running detox
    if app.session.borrow().is_some() {
        return;
    }
    match action {
        Action::Open => show_window(app),
        // The tray menu doesn't offer it during a Pomodoro, but a click may be on its way
        Action::Start(_) if app.pomodoro.borrow().is_some() => {}
        Action::Start(minutes) => start_detox(app, Duration::from_secs(minutes * 60)),
        Action::TogglePomodoro => toggle_pomodoro(app),
        Action::ToggleAutostart => toggle_autostart(app),
        Action::Quit => {
            let _ = slint::quit_event_loop();
        }
    }
}

fn toggle_autostart(app: &App) {
    let enable = !autostart::is_enabled();
    match autostart::set_enabled(enable) {
        Ok(()) => log!("[autostart] start with Windows turned {}", if enable { "on" } else { "off" }),
        Err(e) => log!("[autostart] couldn't change start with Windows: {e}"),
    }
    if let Some(tray) = &app.tray {
        tray.set_autostart(autostart::is_enabled());
    }
}

fn setup_main_window(window: &MainWindow, has_tray: bool) {
    let weak = window.as_weak();
    window.on_minutes_edited(move |text| {
        let Some(window) = weak.upgrade() else { return };
        // Digits only (pasted text included), at most MAX_DIGITS
        let clean: String = text.chars().filter(char::is_ascii_digit).take(MAX_DIGITS).collect();
        if clean != text.as_str() {
            window.set_minutes_text(clean.into());
        }
        window.set_error_text(SharedString::new());
        update_unlock_preview(&window);
    });

    window.on_start_clicked(|| {
        with_app(|app| {
            if app.pomodoro.borrow().is_some() {
                return app.window.set_error_text("Stop the Pomodoro to start a detox".into());
            }
            match parse_minutes(&app.window) {
                Some(minutes) => start_detox(app, Duration::from_secs(minutes * 60)),
                None => app.window.set_error_text("Enter a number of minutes (1 or more)".into()),
            }
        })
    });
    window.on_pomodoro_clicked(|| with_app(toggle_pomodoro));

    // The X hides the window in the notification area instead of quitting.
    // Without a tray icon there would be no way back, so then it quits.
    window.window().on_close_requested(move || {
        if !has_tray {
            let _ = slint::quit_event_loop();
        }
        CloseRequestResponse::HideWindow
    });

    // Windows reports minimizing as a resize to 0x0, and restoring as a resize back
    let minimized = Cell::new(false);
    let weak = window.as_weak();
    window.window().on_winit_window_event(move |_, event| {
        if let WindowEvent::Resized(size) = event {
            if size.width == 0 || size.height == 0 {
                minimized.set(true);
                // Minimizing sends the window to the notification area too
                if has_tray {
                    let weak = weak.clone();
                    Timer::single_shot(Duration::ZERO, move || {
                        if let Some(window) = weak.upgrade() {
                            let _ = window.hide();
                        }
                    });
                }
            } else if minimized.replace(false)
                && let Some(window) = weak.upgrade()
            {
                force_full_repaint(&window);
            }
        }
        EventResult::Propagate
    });

    update_unlock_preview(window);
}

/// Shows the main window (restoring it if it was minimized) and brings it to the front
fn show_window(app: &App) {
    log!("[app] show_window");
    let window = &app.window;
    let _ = window.show();
    window.window().with_winit_window(|w| {
        if w.is_minimized() == Some(true) {
            w.set_minimized(false);
        }
        w.focus_window();
    });
    force_full_repaint(window);
    update_unlock_preview(window);
}

/// Works around a Slint software-renderer bug on Windows: when a window comes back after being
/// minimized or hidden, only the regions that changed are repainted and the rest stays empty.
/// Flipping `repaint-flip` changes a full-window backdrop invisibly, which forces a full repaint.
fn force_full_repaint(window: &MainWindow) {
    window.set_repaint_flip(!window.get_repaint_flip());
}

fn parse_minutes(window: &MainWindow) -> Option<u64> {
    window.get_minutes_text().parse::<u64>().ok().filter(|&minutes| minutes > 0)
}

/// Shows when the detox would end if started now, e.g. "Unlocks at 21:15"
fn update_unlock_preview(window: &MainWindow) {
    let text = match parse_minutes(window) {
        Some(minutes) => {
            let unlock_at = chrono::Local::now() + Duration::from_secs(minutes * 60);
            format!("Unlocks at {}", time_format::short_time(unlock_at))
        }
        None => String::new(),
    };
    window.set_unlock_text(text.into());
}

fn start_detox(app: &Rc<App>, duration: Duration) {
    start_lock(app, duration, SessionKind::Detox);
}

/// Hides the main window and shows the lock screen. When the session ends, the main window comes
/// back only if it was open before (a detox started from the tray menu goes back to the tray).
fn start_lock(app: &Rc<App>, duration: Duration, kind: SessionKind) {
    let reopen = app.window.window().is_visible();
    log!("[app] {kind:?} starting, main window will {}be reopened at the end", if reopen { "" } else { "not " });
    let weak = Rc::downgrade(app);
    let on_end = move || {
        // Called from inside the lock screen's own callbacks: defer, so the session isn't
        // destroyed while one of its callbacks is still running
        let weak = weak.clone();
        Timer::single_shot(Duration::ZERO, move || {
            let Some(app) = weak.upgrade() else { return };
            log!("[app] detox ended, reopen: {reopen}");
            app.session.borrow_mut().take();
            if let Some(tray) = &app.tray {
                tray.set_detox_running(false);
            }
            if kind == SessionKind::Break {
                break_over(&app);
            }
            if reopen {
                show_window(&app);
            }
        });
    };

    match LockSession::start(duration, kind, on_end) {
        Ok(session) => {
            *app.session.borrow_mut() = Some(session);
            if let Some(tray) = &app.tray {
                tray.set_detox_running(true);
            }
            let _ = app.window.hide();
        }
        Err(e) => {
            show_window(app);
            app.window.set_error_text(format!("Couldn't start the lock screen: {e}").into());
            if kind == SessionKind::Break {
                break_over(app);
            }
        }
    }
}

/// Starts the Pomodoro mode, or stops it if it's on
fn toggle_pomodoro(app: &Rc<App>) {
    let stopped = app.pomodoro.borrow_mut().take();
    if stopped.is_some() {
        log!("[pomodoro] stopped");
    } else {
        log!("[pomodoro] started");
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, Duration::from_secs(1), || with_app(pomodoro_tick));
        *app.pomodoro.borrow_mut() = Some(PomodoroRun { cycle: Pomodoro::start(Instant::now()), _timer: timer, reminder: None });
        app.window.set_error_text(SharedString::new());
        app.window.set_tab(1);
    }
    // Dropped here, outside the borrow: this removes the reminder card if it's shown
    drop(stopped);
    if let Some(tray) = &app.tray {
        tray.set_pomodoro_running(app.pomodoro.borrow().is_some());
    }
    update_pomodoro_view(app);
}

fn pomodoro_tick(app: &Rc<App>) {
    let event = {
        let mut slot = app.pomodoro.borrow_mut();
        let Some(run) = slot.as_mut() else { return };
        let event = run.cycle.tick(Instant::now());
        match event {
            Some(pomodoro::Event::Remind) => {
                let minutes = run.cycle.break_length().as_secs() / 60;
                let message = format!("Save your work: the screen will lock for a {minutes}-minute break.");
                run.reminder = Reminder::show(&message).inspect_err(|e| log!("[pomodoro] couldn't show the reminder: {e}")).ok();
            }
            Some(pomodoro::Event::BreakDue(_)) => run.reminder = None,
            None => {}
        }
        event
    };
    if let Some(pomodoro::Event::BreakDue(length)) = event {
        log!("[pomodoro] break of {} minutes", length.as_secs() / 60);
        start_lock(app, length, SessionKind::Break);
    }
    update_pomodoro_view(app);
}

/// A Pomodoro break has ended (or couldn't start): next work round
fn break_over(app: &Rc<App>) {
    if let Some(run) = app.pomodoro.borrow_mut().as_mut() {
        run.cycle.break_over(Instant::now());
    }
    update_pomodoro_view(app);
}

/// Shows the state of the Pomodoro in the main window and in the tray icon's tooltip
fn update_pomodoro_view(app: &App) {
    let window = &app.window;
    let now = Instant::now();
    let slot = app.pomodoro.borrow();
    let tooltip = match slot.as_ref() {
        Some(run) => {
            let round = run.cycle.round();
            let break_minutes = run.cycle.break_length().as_secs() / 60;
            window.set_pomodoro_running(true);
            window.set_pomodoro_round(round as i32);
            match run.cycle.remaining(now) {
                Some(remaining) => {
                    window.set_pomodoro_time(time_format::countdown(remaining).into());
                    window.set_pomodoro_caption(format!("until your {break_minutes}-minute break").into());
                    let break_at = chrono::Local::now() + remaining;
                    window.set_pomodoro_info(format!("Break at {}", time_format::short_time(break_at)).into());
                    let minutes = remaining.as_secs().div_ceil(60);
                    format!("Digital Detox - Pomodoro {round}/{}, break in {minutes} min", pomodoro::ROUNDS)
                }
                None => {
                    window.set_pomodoro_time(time_format::countdown(Duration::ZERO).into());
                    window.set_pomodoro_caption("break in progress".into());
                    window.set_pomodoro_info(SharedString::new());
                    "Digital Detox - Pomodoro break".into()
                }
            }
        }
        None => {
            let break_minutes = pomodoro::SHORT_BREAK.as_secs() / 60;
            window.set_pomodoro_running(false);
            window.set_pomodoro_round(1);
            window.set_pomodoro_time(time_format::countdown(pomodoro::WORK).into());
            window.set_pomodoro_caption(format!("of focus, then a {break_minutes}-minute break").into());
            let break_at = chrono::Local::now() + pomodoro::WORK;
            window.set_pomodoro_info(format!("First break at {}", time_format::short_time(break_at)).into());
            "Digital Detox".into()
        }
    };
    if let Some(tray) = &app.tray {
        tray.set_tooltip(&tooltip);
    }
}
