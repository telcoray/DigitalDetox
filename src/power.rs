//! Screen and power control during a detox: user idle time, switching the monitor off and on,
//! and keeping the PC from going to sleep (which would pause the countdown).

use std::time::Duration;

use slint::winit_030::{WinitWindowAccessor, winit};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, GetPwrCapabilities, SYSTEM_POWER_CAPABILITIES,
    SetThreadExecutionState,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, INPUT, INPUT_0, INPUT_MOUSE, LASTINPUTINFO, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SC_MONITORPOWER, WM_SYSCOMMAND};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// True on PCs with Modern Standby (most recent laptops). There, switching the monitor off is
/// treated like pressing the power button: the PC goes to standby and asks to sign in again.
pub fn has_modern_standby() -> bool {
    let mut capabilities = SYSTEM_POWER_CAPABILITIES::default();
    unsafe { GetPwrCapabilities(&mut capabilities) && capabilities.AoAc }
}

/// How long since the last key press or mouse movement, anywhere in the system
pub fn idle_time() -> Duration {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if GetLastInputInfo(&mut info).as_bool() {
            // Both are milliseconds since boot; wrapping_sub copes with the counter rolling over
            Duration::from_millis(GetTickCount().wrapping_sub(info.dwTime) as u64)
        } else {
            Duration::ZERO
        }
    }
}

/// Switches the monitors off, like Windows does after a few idle minutes.
/// Any mouse movement or key press switches them back on (Windows handles that by itself).
pub fn turn_monitors_off(window: &slint::Window) {
    let Some(hwnd) = window.with_winit_window(hwnd_of).flatten() else {
        return log!("[power] can't switch the monitors off: no native window");
    };
    // 2 = power off. Posted rather than sent, so we never wait on the system to handle it.
    match unsafe { PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(SC_MONITORPOWER as usize), LPARAM(2)) } {
        Ok(()) => log!("[power] monitors off requested"),
        Err(e) => log!("[power] can't switch the monitors off: {e}"),
    }
}

/// Switches the monitors back on, e.g. to show "Detox Complete!" at the end
pub fn wake_monitors() {
    unsafe {
        // Tells Windows the display is needed right now
        SetThreadExecutionState(ES_DISPLAY_REQUIRED);
        // A zero-distance mouse movement counts as user input, which is what reliably
        // turns a monitor back on (the cursor doesn't actually move)
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dwFlags: MOUSEEVENTF_MOVE, ..Default::default() } },
        };
        SendInput(&[input], size_of::<INPUT>() as i32);
    }
}

/// While this value lives, the PC doesn't go to sleep (the display is still allowed to turn off).
pub struct KeepSystemAwake;

impl KeepSystemAwake {
    pub fn new() -> Self {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        }
        Self
    }
}

impl Drop for KeepSystemAwake {
    fn drop(&mut self) {
        // Back to normal: the PC may sleep again according to the user's power settings
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

pub(crate) fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
        _ => None,
    }
}
