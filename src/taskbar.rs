//! Keeps the Windows taskbar out of the way of the lock screen and the overlays.
//!
//! Windows only hides the taskbar in front of a full-screen window when that window is the
//! active one. The overlays on the secondary monitors never are (the lock screen keeps the
//! focus), so their taskbar stayed visible. `MarkFullscreenWindow` tells the taskbar that a
//! window is full screen regardless of focus.

use slint::winit_030::WinitWindowAccessor;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Shell::{ITaskbarList2, TaskbarList};

use crate::power::hwnd_of;

/// Must be called after the window is shown (the native window has to exist)
pub fn mark_fullscreen(window: &slint::Window) {
    let Some(hwnd) = window.with_winit_window(hwnd_of).flatten() else { return };
    unsafe {
        // COM is usually already set up on the UI thread; this is a no-op then
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = CoCreateInstance::<_, ITaskbarList2>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
            .and_then(|taskbar| taskbar.HrInit().map(|()| taskbar))
            .and_then(|taskbar| taskbar.MarkFullscreenWindow(hwnd, true));
        if let Err(e) = result {
            log!("[taskbar] couldn't mark the window as full screen: {e}");
        }
    }
}
