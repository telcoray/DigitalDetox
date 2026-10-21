//! One detox session: the lock screen, the overlays on the other monitors, the countdown
//! timer, the keyboard hook and the gradual dimming of the screen.
//! Everything is torn down when the `LockSession` is dropped.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{CloseRequestResponse, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use crate::brightness;
use crate::foreground::ForegroundGuard;
use crate::keyboard_hook::KeyboardHook;
use crate::taskbar::{self, HiddenTaskbars};
use crate::power::{self, KeepSystemAwake};
use crate::screen::{self, ScreenControl, Shade};
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
    // The PC doesn't sleep during the detox, or the countdown would pause
    _awake: KeepSystemAwake,
    // No taskbar (nor Start button) to click during the detox. Shared with the timer, which
    // hides it again when Start brings it back.
    _taskbars: Rc<HiddenTaskbars>,
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
        window.set_unlock_text(format!("Unlocks at: {}", crate::time_format::long_time(unlock_at)).into());
        update_display(&window, &[], end, duration);

        // Ignore Alt+F4 and other close requests: the window closes only when the detox ends
        window.window().on_close_requested(|| CloseRequestResponse::KeepWindowShown);
        window.on_exit_requested({
            let on_end = on_end.clone();
            move || on_end()
        });
        setup_emergency_exit(&window);
        keep_focus(&window);

        // Gradual switching off of the screen (see screen.rs)
        let screen = Rc::new(RefCell::new(ScreenControl::new()));
        window.on_turn_off_screen({
            let weak = window.as_weak();
            let covers = covers.clone();
            let screen = screen.clone();
            move || {
                if let Some(window) = weak.upgrade() {
                    let shade = screen.borrow_mut().turn_off_now();
                    apply_shade(&window, &covers.borrow(), shade);
                }
            }
        });
        window.on_more_time({
            let weak = window.as_weak();
            let screen = screen.clone();
            move || {
                screen.borrow_mut().more_time();
                if let Some(window) = weak.upgrade() {
                    window.set_more_time_visible(false);
                }
            }
        });

        // Same approach as the C# version: tick every 250 ms and recompute from the end time
        let timer = Timer::default();
        let weak = window.as_weak();
        let timer_covers = covers.clone();
        let foreground = ForegroundGuard::default();
        let taskbars = Rc::new(HiddenTaskbars::hide());
        let timer_taskbars = taskbars.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            let Some(window) = weak.upgrade() else { return };
            if window.get_completed() {
                return;
            }
            // Start or another app may have taken the focus (see foreground.rs)
            foreground.check(window.window());
            timer_taskbars.keep_hidden();
            let covers = timer_covers.borrow();
            if update_display(&window, &covers, end, duration) {
                complete_detox(&window, &covers, on_end.clone());
            } else {
                let mut screen = screen.borrow_mut();
                if let Some(shade) = screen.tick(window.window()) {
                    apply_shade(&window, &covers, shade);
                }
                window.set_more_time_visible(screen.offers_more_time());
            }
        });

        // Right after the lock screen appears, gently dim it and lower the laptop's brightness:
        // the first signal of "switching off"
        let weak = window.as_weak();
        let dim_covers = covers.clone();
        Timer::single_shot(Duration::from_millis(500), move || {
            if let Some(window) = weak.upgrade() {
                apply_shade(&window, &dim_covers.borrow(), Shade { level: screen::DIM_LEVEL, fade: screen::DIM_FADE });
                brightness::lower();
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
            .inspect_err(|e| log!("Could not install the keyboard hook: {e}"))
            .ok();

        window.window().set_fullscreen(true);
        window.show()?;

        Ok(Self {
            window,
            covers,
            _timer: timer,
            _hook: hook,
            _awake: KeepSystemAwake::new(),
            _taskbars: taskbars,
        })
    }
}

impl Drop for LockSession {
    fn drop(&mut self) {
        for cover in self.covers.borrow().iter() {
            let _ = cover.hide();
        }
        let _ = self.window.hide();
        // Also covers the emergency exit; does nothing if it was already restored at the end
        brightness::restore();
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
    // The detox is over: monitors back on, screen back to full brightness
    power::wake_monitors();
    brightness::restore();
    apply_shade(window, covers, Shade { level: 0.0, fade: Duration::from_millis(600) });
    // Leave "Detox Complete!" on screen for 5 seconds (monitors can take a moment to wake up),
    // then end the session
    Timer::single_shot(Duration::from_secs(5), move || on_end());
}

/// Dims, darkens or brightens the lock screen and every cover window together
fn apply_shade(window: &LockWindow, covers: &[CoverWindow], shade: Shade) {
    // Slint durations are milliseconds. The fade is set first, so the level change uses it.
    let fade = shade.fade.as_millis() as i64;
    window.set_shade_fade(fade);
    window.set_shade_level(shade.level);
    for cover in covers {
        cover.set_shade_fade(fade);
        cover.set_shade_level(shade.level);
    }
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
    taskbar::mark_fullscreen(window.window());
    log!("[session] {} secondary monitor(s) to cover", secondary.as_ref().map_or(0, Vec::len));

    for monitor in secondary.unwrap_or_default() {
        match create_cover(&monitor, window) {
            Ok(cover) => covers.push(cover),
            Err(e) => log!("Could not cover monitor {:?}: {e}", monitor.name()),
        }
    }
}

fn create_cover(monitor: &winit::monitor::MonitorHandle, lock: &LockWindow) -> Result<CoverWindow, slint::PlatformError> {
    let cover = CoverWindow::new()?;
    cover.set_time_chars(lock.get_time_chars());
    // Start as dark as the lock screen currently is
    cover.set_shade_level(lock.get_shade_level());
    cover.window().on_close_requested(|| CloseRequestResponse::KeepWindowShown);

    // Open the window on its monitor, then make it full screen there
    let position = monitor.position();
    cover.window().set_position(slint::PhysicalPosition::new(position.x, position.y));
    cover.window().set_fullscreen(true);
    cover.show()?;
    cover.window().with_winit_window(|w| {
        w.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(monitor.clone()))));
    });
    // Otherwise the taskbar of that monitor stays visible on top (see taskbar.rs)
    taskbar::mark_fullscreen(cover.window());
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

/// Picks a message without pulling in a random-number crate: the clock's nanoseconds are random enough here
fn random_message() -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    MESSAGES[nanos % MESSAGES.len()]
}
