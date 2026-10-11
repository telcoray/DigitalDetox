//! Blocks system shortcuts while the lock screen is up: Windows key, Alt+Tab, Alt+F4,
//! Alt+Esc and Ctrl+Esc (which also covers Ctrl+Shift+Esc, the Task Manager shortcut).
//!
//! Same technique as the C# version: a low-level keyboard hook (WH_KEYBOARD_LL) that sees every
//! key press system-wide before any application does, and can swallow it.
//! Ctrl+Alt+Del is handled by Windows itself and can't be intercepted by any application.

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_F4, VK_LWIN, VK_RWIN, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

/// Active keyboard hook. The hook is removed automatically when this value is dropped,
/// so it can't be left behind even if the app exits early or panics.
pub struct KeyboardHook(HHOOK);

impl KeyboardHook {
    /// Installs the hook. Windows delivers its callbacks through the message loop of the
    /// calling thread, so this must be called on the UI thread (Slint's event loop pumps it).
    pub fn install() -> windows::core::Result<Self> {
        unsafe {
            let module = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(HINSTANCE(module.0)), 0)?;
            Ok(Self(hook))
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}

/// Called by Windows for every key event. It must return quickly: if it takes too long,
/// Windows silently removes the hook.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
            // For WH_KEYBOARD_LL, lparam points to a KBDLLHOOKSTRUCT describing the key
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if should_block(info) {
                // A non-zero result swallows the key: no application, nor Windows, sees it
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn should_block(info: &KBDLLHOOKSTRUCT) -> bool {
    let key = info.vkCode as u16;
    let alt = info.flags.0 & LLKHF_ALTDOWN.0 != 0;
    // The high bit of GetAsyncKeyState is set while the key is held down
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } < 0;

    key == VK_LWIN.0
        || key == VK_RWIN.0
        || (alt && (key == VK_TAB.0 || key == VK_F4.0 || key == VK_ESCAPE.0))
        || (ctrl && key == VK_ESCAPE.0)
}
