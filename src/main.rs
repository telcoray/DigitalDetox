// In release builds, don't open a console window next to the app
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, SharedString, Timer, TimerMode};

mod brightness;
mod keyboard_hook;
mod power;
mod screen;
mod session;
mod single_instance;
mod time_format;

use session::LockSession;

// Brings in the Rust types generated from ui/app.slint (`MainWindow`, `LockWindow`, `CoverWindow`)
slint::include_modules!();

/// At most 3 digits (so up to 999 minutes)
const MAX_DIGITS: usize = 3;

fn main() -> Result<(), slint::PlatformError> {
    // Held until main returns, so no second copy can start while this one runs
    let Ok(_instance) = single_instance::SingleInstance::acquire() else {
        single_instance::show_already_running();
        return Ok(());
    };

    // If a previous run was closed abruptly during a detox, give the screen its brightness back
    brightness::recover();

    let main_window = MainWindow::new()?;
    // The running session, if any. Dropping it closes the lock screen.
    let session: Rc<RefCell<Option<LockSession>>> = Rc::default();

    let weak = main_window.as_weak();
    main_window.on_minutes_edited(move |text| {
        let Some(window) = weak.upgrade() else { return };
        // Digits only (pasted text included), at most MAX_DIGITS
        let clean: String = text.chars().filter(char::is_ascii_digit).take(MAX_DIGITS).collect();
        if clean != text.as_str() {
            window.set_minutes_text(clean.into());
        }
        window.set_error_text(SharedString::new());
        update_unlock_preview(&window);
    });

    let weak = main_window.as_weak();
    let start_session = session.clone();
    main_window.on_start_clicked(move || {
        let Some(window) = weak.upgrade() else { return };
        match parse_minutes(&window) {
            Some(minutes) => start_detox(&window, &start_session, Duration::from_secs(minutes * 60)),
            None => window.set_error_text("Enter a number of minutes (1 or more)".into()),
        }
    });

    update_unlock_preview(&main_window);
    // The clock moves on while the window is open: keep "Unlocks at …" current
    let preview_timer = Timer::default();
    let weak = main_window.as_weak();
    preview_timer.start(TimerMode::Repeated, Duration::from_secs(5), move || {
        if let Some(window) = weak.upgrade() {
            update_unlock_preview(&window);
        }
    });

    main_window.run()
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

/// Swaps the main window for the lock screen, and back again when the session ends.
fn start_detox(main_window: &MainWindow, slot: &Rc<RefCell<Option<LockSession>>>, duration: Duration) {
    let weak = main_window.as_weak();
    let end_slot = slot.clone();
    let on_end = move || {
        // Called from inside the lock screen's own callbacks: defer, so the session isn't
        // destroyed while one of its callbacks is still running
        let weak = weak.clone();
        let slot = end_slot.clone();
        Timer::single_shot(Duration::ZERO, move || {
            // Show the main window before closing the lock screen: if the app had no window
            // open even for a moment, Slint would consider it finished and exit
            if let Some(window) = weak.upgrade() {
                let _ = window.show();
                update_unlock_preview(&window);
            }
            slot.borrow_mut().take();
        });
    };

    match LockSession::start(duration, on_end) {
        Ok(session) => {
            *slot.borrow_mut() = Some(session);
            // Same reason as above: hide the main window only once the lock screen is open
            let _ = main_window.hide();
        }
        Err(e) => main_window.set_error_text(format!("Couldn't start the lock screen: {e}").into()),
    }
}
