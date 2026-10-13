//! Keeps the lock screen the active window for the whole detox.
//!
//! `keep_focus` in session.rs reacts when the lock screen loses focus, but only once: Start
//! opens asynchronously and took the focus again right after. So the countdown tick also checks
//! which window is active whenever it belongs to another process:
//! - Start, Search and the other shell flyouts don't give the foreground away, not even with the
//!   Alt-key trick, so they get an Escape key press, which closes them;
//! - any other window: the lock screen takes the foreground back.

use std::cell::Cell;

use slint::winit_030::WinitWindowAccessor;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId};

/// The window class of Start, Search, the notification center and the other shell flyouts
const SHELL_FLYOUT_CLASS: &str = "Windows.UI.Core.CoreWindow";

#[derive(Default)]
pub struct ForegroundGuard {
    // The foreign window logged last, so the log gets one line per window rather than one per tick
    last_logged: Cell<usize>,
}

impl ForegroundGuard {
    pub fn check(&self, lock: &slint::Window) {
        let foreground = unsafe { GetForegroundWindow() };
        // No active window happens briefly while Windows switches between windows
        if foreground.is_invalid() || belongs_to_this_app(foreground) {
            self.last_logged.set(0);
            return;
        }
        let class = class_name(foreground);
        if self.last_logged.replace(foreground.0 as usize) != foreground.0 as usize {
            log!("[focus] {class} took the foreground, taking it back");
        }
        if class == SHELL_FLYOUT_CLASS {
            press_escape();
        } else {
            // winit's focus_window uses the Alt-key trick that SetForegroundWindow needs to work from the background
            lock.with_winit_window(|w| w.focus_window());
        }
    }
}

/// Sends an Escape key press to the active window. The keyboard hook lets a plain Escape through.
fn press_escape() {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_ESCAPE, dwFlags: flags, ..Default::default() } },
    };
    let inputs = [key(Default::default()), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}

fn belongs_to_this_app(hwnd: HWND) -> bool {
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    process_id == unsafe { GetCurrentProcessId() }
}

/// The window class, e.g. "Windows.UI.Core.CoreWindow" for Start: tells in the log what took over
fn class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..len.max(0) as usize])
}
