//! One detox session: the lock screen, the overlays on the other monitors, the countdown
//! timer and the keyboard hook. Everything is torn down when the `LockSession` is dropped.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{CloseRequestResponse, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use crate::keyboard_hook::KeyboardHook;
use crate::{CoverWindow, LockWindow};

const MESSAGES: [&str; 10] = [
    "Take a deep breath. Relax your shoulders.",
    "Your mind deserves this break.",
    "Disconnect to reconnect with yourself.",
    "Breathe in. Breathe out. Just be.",
    "No notifications. No pressure. Just peace.",
    "This time is yours alone.",
    "Let your eyes rest. Let your mind wander.",
    "You're doing great. Keep going.",
    "The world can wait. You matter.",
    "Find calm in the stillness.",
];

/// Ctrl+Shift+X must be pressed this many times, each within `EMERGENCY_WINDOW` of the previous one
const EMERGENCY_PRESSES: u32 = 3;
const EMERGENCY_WINDOW: Duration = Duration::from_secs(2);

pub struct LockSession {
    window: LockWindow,
    // Overlays for the secondary monitors, created once the lock screen exists (see `start`).
    // Rc<RefCell<..>>: shared with the timer and the setup code, and mutable from either.
    covers: Rc<RefCell<Vec<CoverWindow>>>,
    _timer: Timer,
    // Blocks Win, Alt+Tab & co. for as long as the session lives
    _hook: Option<KeyboardHook>,
}

impl LockSession {
    /// Shows the lock screen for `duration`. `on_end` is called when the session should be
    /// closed: 3 seconds after the timer runs out, or after a confirmed emergency exit.
    pub fn start(duration: Duration, on_end: impl Fn() + 'static) -> Result<Self, slint::PlatformError> {
        let on_end: Rc<dyn Fn()> = Rc::new(on_end);
        // Instant is a monotonic clock: unaffected by clock changes or daylight saving time
        let end = Instant::now() + duration;

        let window = LockWindow::new()?;
        let covers: Rc<RefCell<Vec<CoverWindow>>> = Rc::default();

        window.set_motivation(random_message().into());
        let unlock_at = chrono::Local::now() + duration;
        window.set_unlock_text(format!("Unlocks at: {}", format_local_time(unlock_at)).into());
        update_display(&window, &[], end, duration);

        // Ignore Alt+F4 and other close requests: the window closes only when the detox ends
        window.window().on_close_requested(|| CloseRequestResponse::KeepWindowShown);
        window.on_exit_requested({
            let on_end = on_end.clone();
            move || on_end()
        });
        setup_emergency_exit(&window);
        keep_focus(&window);

        // Same approach as the C# version: tick every 250 ms and recompute from the end time
        let timer = Timer::default();
        let weak = window.as_weak();
        let timer_covers = covers.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let Some(window) = weak.upgrade() else { return };
            if window.get_completed() {
                return;
            }
            let covers = timer_covers.borrow();
            if update_display(&window, &covers, end, duration) {
                complete_detox(&window, &covers, on_end.clone());
            }
        });

        // Monitors can only be listed once the native window exists: a zero-delay timer
        // runs this on the next iteration of the event loop, after `show()` below
        let weak = window.as_weak();
        let setup_covers = covers.clone();
        Timer::single_shot(Duration::ZERO, move || {
            if let Some(window) = weak.upgrade() {
                cover_secondary_monitors(&window, &mut setup_covers.borrow_mut());
            }
        });

        // If the hook can't be installed the lock screen still works, just without the shortcut blocking
        let hook = KeyboardHook::install()
            .inspect_err(|e| eprintln!("Could not install the keyboard hook: {e}"))
            .ok();

        window.window().set_fullscreen(true);
        window.show()?;

        Ok(Self { window, covers, _timer: timer, _hook: hook })
    }
}

impl Drop for LockSession {
    fn drop(&mut self) {
        for cover in self.covers.borrow().iter() {
            let _ = cover.hide();
        }
        let _ = self.window.hide();
        // The fields are dropped right after this: the timer stops and the keyboard hook is removed
    }
}

/// Refreshes timer and progress bar on every window. Returns true once the time is up.
fn update_display(window: &LockWindow, covers: &[CoverWindow], end: Instant, total: Duration) -> bool {
    let remaining = end.saturating_duration_since(Instant::now());

    // Round up so the display only reaches 00:00 when the detox actually ends
    let secs = remaining.as_secs_f64().ceil() as u64;
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let text = if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    };
    // The UI lays out each character in its own fixed-width cell, so pass them one by one
    let chars: Vec<SharedString> = text.chars().map(|c| c.to_string().into()).collect();
    let chars = ModelRc::new(VecModel::from(chars));
    for cover in covers {
        cover.set_time_chars(chars.clone());
    }
    window.set_time_chars(chars);

    let progress = 1.0 - remaining.as_secs_f64() / total.as_secs_f64();
    window.set_progress(progress.clamp(0.0, 1.0) as f32);

    remaining.is_zero()
}

fn complete_detox(window: &LockWindow, covers: &[CoverWindow], on_end: Rc<dyn Fn()>) {
    window.set_completed(true);
    window.set_progress(1.0);
    for cover in covers {
        cover.set_completed(true);
    }
    // Leave the "Detox Complete!" message on screen for 3 seconds, then end the session
    Timer::single_shot(Duration::from_secs(3), move || on_end());
}

/// Puts the lock screen on the primary monitor and a `CoverWindow` on every other one.
fn cover_secondary_monitors(window: &LockWindow, covers: &mut Vec<CoverWindow>) {
    let secondary = window.window().with_winit_window(|w| {
        let primary = w.primary_monitor();
        w.set_fullscreen(Some(winit::window::Fullscreen::Borderless(primary.clone())));
        w.available_monitors()
            .filter(|m| Some(m) != primary.as_ref())
            .collect::<Vec<_>>()
    });

    for monitor in secondary.unwrap_or_default() {
        match create_cover(&monitor, window) {
            Ok(cover) => covers.push(cover),
            Err(e) => eprintln!("Could not cover monitor {:?}: {e}", monitor.name()),
        }
    }
}

fn create_cover(monitor: &winit::monitor::MonitorHandle, lock: &LockWindow) -> Result<CoverWindow, slint::PlatformError> {
    let cover = CoverWindow::new()?;
    cover.set_time_chars(lock.get_time_chars());
    cover.window().on_close_requested(|| CloseRequestResponse::KeepWindowShown);

    // Open the window on its monitor, then make it full screen there
    let position = monitor.position();
    cover.window().set_position(slint::PhysicalPosition::new(position.x, position.y));
    cover.window().set_fullscreen(true);
    cover.show()?;
    cover.window().with_winit_window(|w| {
        w.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(monitor.clone()))));
    });
    Ok(cover)
}

/// Whenever the lock screen loses focus (a click on another monitor, Ctrl+Alt+Del → Task Manager…),
/// take it back right away, like the Deactivated handler of the C# version.
fn keep_focus(window: &LockWindow) {
    let weak = window.as_weak();
    window.window().on_winit_window_event(move |_, event| {
        if matches!(event, winit::event::WindowEvent::Focused(false)) {
            // Deferred to the next loop iteration, so Windows has finished switching windows
            let weak = weak.clone();
            Timer::single_shot(Duration::ZERO, move || {
                if let Some(window) = weak.upgrade().filter(|w| !w.get_completed()) {
                    window.window().with_winit_window(|w| w.focus_window());
                }
            });
        }
        EventResult::Propagate
    });
}

/// Counts Ctrl+Shift+X presses; after three in a row the Emergency Exit button appears.
fn setup_emergency_exit(window: &LockWindow) {
    // (presses so far, time of the last press). Cell allows updating it from inside the callback.
    let presses = Cell::new((0u32, None::<Instant>));
    let weak = window.as_weak();
    window.on_emergency_key_pressed(move || {
        let Some(window) = weak.upgrade() else { return };
        let now = Instant::now();
        let (count, last) = presses.get();
        let count = match last {
            Some(t) if now - t < EMERGENCY_WINDOW => count + 1,
            _ => 1,
        };

        if count >= EMERGENCY_PRESSES {
            window.set_emergency_visible(true);
            window.set_status_message(SharedString::new());
            presses.set((0, None));
        } else {
            let left = EMERGENCY_PRESSES - count;
            window.set_status_message(format!("Emergency exit: Press Ctrl+Shift+X {left} more time(s)").into());
            presses.set((count, Some(now)));
        }
    });
}

/// Formats a time the way the user's Windows settings say (like .NET's "T" format in the C# version):
/// 19:19:33 with Italian settings, 7:19:33 PM with US settings.
fn format_local_time(time: chrono::DateTime<chrono::Local>) -> String {
    use chrono::Timelike;
    use windows::Win32::Foundation::SYSTEMTIME;
    use windows::Win32::Globalization::{GetTimeFormatEx, TIME_FORMAT_FLAGS};
    use windows::core::PCWSTR;

    // Only the time fields matter: GetTimeFormatEx ignores the date
    let system_time = SYSTEMTIME {
        wHour: time.hour() as u16,
        wMinute: time.minute() as u16,
        wSecond: time.second() as u16,
        ..Default::default()
    };
    let mut buffer = [0u16; 64];
    // Null locale and format = the current user's locale and its default time format
    let len = unsafe {
        GetTimeFormatEx(PCWSTR::null(), TIME_FORMAT_FLAGS(0), Some(&system_time), PCWSTR::null(), Some(&mut buffer))
    };
    if len > 1 {
        // `len` counts the terminating zero
        String::from_utf16_lossy(&buffer[..len as usize - 1])
    } else {
        time.format("%H:%M:%S").to_string()
    }
}

/// Picks a message without pulling in a random-number crate: the clock's nanoseconds are random enough here
fn random_message() -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    MESSAGES[nanos % MESSAGES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn local_time_uses_windows_format() {
        let time = chrono::Local.with_ymd_and_hms(2026, 10, 11, 19, 19, 33).unwrap();
        let text = format_local_time(time);
        println!("formatted: {text}");
        // 24-hour ("19:19:33") or 12-hour ("7:19:33 PM") depending on the PC's settings
        assert!(text.contains("19:33"), "unexpected format: {text}");
    }
}
