// In release builds, don't open a console window next to the app
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::{WinitWindowAccessor, winit};
use slint::{ComponentHandle, SharedString, Timer};

mod keyboard_hook;
mod session;
mod single_instance;

use session::LockSession;

// Brings in the Rust types generated from ui/app.slint (`MainWindow`, `LockWindow`, `CoverWindow`)
slint::include_modules!();

/// At most 3 digits, like MaxLength="3" in the C# version (so up to 999 minutes)
const MAX_DIGITS: usize = 3;

fn main() -> Result<(), slint::PlatformError> {
    // Held until main returns, so no second copy can start while this one runs
    let Ok(_instance) = single_instance::SingleInstance::acquire() else {
        single_instance::show_already_running();
        return Ok(());
    };

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
    });

    let weak = main_window.as_weak();
    let start_session = session.clone();
    main_window.on_start_clicked(move || {
        let Some(window) = weak.upgrade() else { return };
        match window.get_minutes_text().parse::<u64>() {
            Ok(minutes) if minutes > 0 => {
                start_detox(&window, &start_session, Duration::from_secs(minutes * 60));
            }
            _ => window.set_error_text("Please enter a valid number of minutes (1 or more).".into()),
        }
    });

    // The native window only exists once the event loop runs: adjust it on the first iteration
    let weak = main_window.as_weak();
    Timer::single_shot(Duration::ZERO, move || {
        if let Some(window) = weak.upgrade() {
            only_close_button(&window);
        }
    });

    main_window.run()
}

/// Keeps just the X in the title bar, like the C# window (no minimize or maximize button).
/// winit can only grey the buttons out; removing them takes the window styles from Win32 directly.
fn only_close_button(window: &MainWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SetWindowLongPtrW, SetWindowPos, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    window.window().with_winit_window(|w| {
        let Ok(handle) = w.window_handle() else { return };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let buttons = (WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) as isize;
            SetWindowLongPtrW(hwnd, GWL_STYLE, style & !buttons);
            // Tell Windows to redraw the title bar with the new style
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
        }
    });
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
                only_close_button(&window);
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
        Err(e) => main_window.set_error_text(format!("Could not start the lock screen: {e}").into()),
    }
}
