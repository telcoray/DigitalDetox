//! Keeps the Windows taskbar out of the way of the lock screen and the overlays.
//!
//! Windows only hides the taskbar in front of a full-screen window when that window is the
//! active one. The overlays on the secondary monitors never are (the lock screen keeps the
//! focus), so their taskbar stayed visible. `MarkFullscreenWindow` tells the taskbar that a
//! window is full screen regardless of focus.
//!
//! On recent Windows 11 builds that isn't enough: the Windows key opens Start and brings up the
//! taskbar even though the keyboard hook swallows it, and its icons can be clicked. So during a
//! detox the taskbars are also hidden outright (`HiddenTaskbars`).

use slint::winit_030::WinitWindowAccessor;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::core::BOOL;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Shell::{ITaskbarList2, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetWindowThreadProcessId, HWND_BROADCAST, IsWindowVisible, SMTO_ABORTIFHUNG,
    SW_HIDE, SW_SHOW, SendMessageTimeoutW, ShowWindow, WM_SETTINGCHANGE,
};
use windows::core::{PCWSTR, w};

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

/// The taskbars of every monitor, hidden while this value lives and shown again when it's dropped.
/// A marker file records that they're hidden, so `recover` can bring them back if the app is
/// closed abruptly.
///
/// The taskbars are looked up again each time: when a monitor is disconnected and reconnected
/// (which waking it up can do), Explorer gives it a new taskbar.
pub struct HiddenTaskbars;

impl HiddenTaskbars {
    pub fn hide() -> Self {
        let taskbars = find_taskbars();
        let _ = std::fs::create_dir_all(crate::log::data_dir());
        let _ = std::fs::write(marker_path(), "");
        for &hwnd in &taskbars {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
        log!("[taskbar] {} taskbar(s) hidden", taskbars.len());
        Self
    }

    /// Explorer shows the taskbar again whenever Start opens, or creates a new one for a monitor
    /// that comes back: hide it once more
    pub fn keep_hidden(&self) {
        for hwnd in find_taskbars() {
            unsafe {
                if IsWindowVisible(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }
}

impl Drop for HiddenTaskbars {
    fn drop(&mut self) {
        show(&find_taskbars());
    }
}

/// At start-up: if a previous run ended with the taskbars hidden, show them again
pub fn recover() {
    if marker_path().exists() {
        log!("[taskbar] the previous run left the taskbars hidden, showing them");
        show(&find_taskbars());
    }
}

/// From the panic hook, when the app is about to go down in the middle of a detox. The lock
/// screen is hidden first, as at the normal end of a detox: a taskbar shown while a full-screen
/// window is still there stays on screen, even with auto-hide on.
pub fn recover_after_panic() {
    if marker_path().exists() {
        hide_own_windows();
        recover();
    }
}

fn hide_own_windows() {
    unsafe extern "system" fn hide_if_ours(hwnd: HWND, _: LPARAM) -> BOOL {
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process));
            if process == std::process::id() && IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
        true.into()
    }
    unsafe {
        let _ = EnumWindows(Some(hide_if_ours), LPARAM(0));
    }
}

fn show(taskbars: &[HWND]) {
    for &hwnd in taskbars {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }
    refresh_taskbar_settings();
    let _ = std::fs::remove_file(marker_path());
}

/// With "Automatically hide the taskbar" on, a taskbar shown again with ShowWindow stays on
/// screen, and on secondary monitors an invisible strip keeps catching the clicks.
/// "TraySettings" asks Explorer to read the taskbar settings again, which lays them out afresh,
/// auto-hide included. (Asking for the auto-hide state through SHAppBarMessage doesn't work on
/// recent Windows 11 builds: it always answers "off".)
fn refresh_taskbar_settings() {
    let mut result = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(w!("TraySettings").as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        );
    }
}

/// The main taskbar, plus one per secondary monitor
fn find_taskbars() -> Vec<HWND> {
    let mut taskbars = Vec::new();
    unsafe {
        if let Ok(main) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            taskbars.push(main);
        }
        // FindWindowExW continues the search after the window found last time
        let mut after = None;
        while let Ok(hwnd) = FindWindowExW(None, after, w!("Shell_SecondaryTrayWnd"), PCWSTR::null()) {
            taskbars.push(hwnd);
            after = Some(hwnd);
        }
    }
    taskbars
}

fn marker_path() -> std::path::PathBuf {
    crate::log::data_dir().join("taskbar-hidden")
}
