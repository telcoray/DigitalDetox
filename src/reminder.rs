//! The reminder a minute before a Pomodoro break: a small card in the bottom-right corner of the
//! primary monitor, just above the taskbar. It must not take the focus away from whatever you're
//! typing in.

use std::time::Duration;

use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, SPI_GETWORKAREA, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, SystemParametersInfoW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
};

use crate::ReminderWindow;
use crate::power::hwnd_of;

/// Same size as in reminder.slint, in logical pixels
const SIZE: (f32, f32) = (340.0, 88.0);
/// Distance from the edges of the screen (and from the taskbar), in logical pixels
const MARGIN: f32 = 12.0;

/// The card disappears when this is dropped
pub struct Reminder(ReminderWindow);

impl Reminder {
    pub fn show(message: &str) -> Result<Self, slint::PlatformError> {
        let window = ReminderWindow::new()?;
        window.set_message(message.into());
        let weak = window.as_weak();
        window.on_dismissed(move || {
            if let Some(window) = weak.upgrade() {
                let _ = window.hide();
            }
        });

        let previous = unsafe { GetForegroundWindow() };
        // A first guess, so the window is created on the primary monitor and not elsewhere
        if let Some(area) = work_area() {
            let scale = unsafe { GetDpiForSystem() } as f32 / 96.0;
            let x = area.right - ((SIZE.0 + MARGIN) * scale).round() as i32;
            let y = area.bottom - ((SIZE.1 + MARGIN) * scale).round() as i32;
            window.window().set_position(slint::PhysicalPosition::new(x, y));
        }
        window.show()?;
        window.window().with_winit_window(|w| {
            let Some(hwnd) = hwnd_of(w) else { return };
            unsafe {
                // No taskbar button, and clicking it doesn't make it the active window
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) as isize);
                // Shown again so the new styles apply, this time without activating it
                let _ = ShowWindow(hwnd, SW_HIDE);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                // If showing it took the focus anyway, give it back
                if GetForegroundWindow() == hwnd && !previous.is_invalid() {
                    let _ = SetForegroundWindow(previous);
                }
            }
            move_to_corner(hwnd);
        });
        // The window may still be resized for the monitor's scale right after it appears:
        // place it again once that has happened
        let weak = window.as_weak();
        Timer::single_shot(Duration::from_millis(200), move || {
            if let Some(window) = weak.upgrade() {
                window.window().with_winit_window(|w| hwnd_of(w).map(move_to_corner));
            }
        });
        Ok(Self(window))
    }
}

impl Drop for Reminder {
    fn drop(&mut self) {
        let _ = self.0.hide();
    }
}

/// The primary monitor minus the taskbar, in physical pixels
fn work_area() -> Option<RECT> {
    let mut area = RECT::default();
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut area as *mut RECT as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
    }
    .ok()
    .map(|()| area)
}

/// Bottom-right corner of the work area, using the window's actual size in pixels
fn move_to_corner(hwnd: HWND) {
    let Some(area) = work_area() else { return };
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let margin = (MARGIN * unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0).round() as i32;
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let (x, y) = (area.right - width - margin, area.bottom - height - margin);
    log!(
        "[pomodoro] reminder {width}x{height} at {x},{y} (work area {},{} - {},{})",
        area.left, area.top, area.right, area.bottom
    );
    unsafe {
        let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}
