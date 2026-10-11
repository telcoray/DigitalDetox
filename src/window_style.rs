//! Title bar tweaks for the main window that winit doesn't offer, done through Win32 directly.
//! The title bar colors are left to Windows, so they follow the system light/dark theme.

use slint::winit_030::{WinitWindowAccessor, winit};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowLongPtrW, SetWindowPos, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Keeps only the close button in the title bar (no minimize or maximize).
/// winit can only grey those buttons out; removing them takes changing the window style.
/// Must be called once the native window exists (i.e. after the event loop has started).
pub fn style_main_window(window: &slint::Window) {
    window.with_winit_window(|w| {
        let Some(hwnd) = hwnd_of(w) else { return };
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let buttons = (WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) as isize;
            SetWindowLongPtrW(hwnd, GWL_STYLE, style & !buttons);
            // Tell Windows to redraw the title bar with the new style
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
        }
    });
}

fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
        _ => None,
    }
}
