//! Blocks system shortcuts while the lock screen is up: Windows key, Alt+Tab, Alt+F4,
//! Alt+Esc and Ctrl+Esc (which also covers Ctrl+Shift+Esc, the Task Manager shortcut).
//!
//! Same technique as the C# version: a low-level keyboard hook (WH_KEYBOARD_LL) that sees every
//! key press system-wide before any application does, and can swallow it.
//! Ctrl+Alt+Del is handled by Windows itself and can't be intercepted by any application.

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_ESCAPE, VK_F4, VK_LWIN, VK_RWIN, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, MSG, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_QUIT, WM_SYSKEYDOWN,
};

/// Active keyboard hook. The hook is removed automatically when this value is dropped,
/// so it can't be left behind even if the app exits early or panics.
pub struct KeyboardHook {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl KeyboardHook {
    /// Installs the hook on a dedicated thread. Windows delivers the callbacks through the
    /// message loop of the installing thread and lets a key through anyway if the callback
    /// doesn't answer in time: on the UI thread, a slow repaint was enough for the Windows key
    /// and Alt+Tab to slip past. This thread does nothing else, so it always answers at once.
    pub fn install() -> windows::core::Result<Self> {
        // The new thread reports back whether the hook was installed, and its thread id
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::spawn(move || unsafe {
            let hook = GetModuleHandleW(None)
                .and_then(|module| SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(HINSTANCE(module.0)), 0));
            let hook = match hook {
                Ok(hook) => hook,
                Err(e) => {
                    let _ = sender.send(Err(e));
                    return;
                }
            };
            // Makes sure the thread has a message queue before Drop can post WM_QUIT to it
            let mut msg = MSG::default();
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            let _ = sender.send(Ok(GetCurrentThreadId()));

            // Pumping messages is what makes Windows call hook_proc. GetMessageW returns 0 on
            // WM_QUIT and -1 on error: both end the loop.
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}
            let _ = UnhookWindowsHookEx(hook);
        });

        match receiver.recv() {
            Ok(Ok(thread_id)) => Ok(Self { thread_id, thread: Some(thread) }),
            Ok(Err(e)) => Err(e),
            // The thread ended without answering (it panicked)
            Err(_) => Err(windows::core::Error::from_thread()),
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Called by Windows for every key event. It must return quickly: if it takes too long,
/// Windows silently removes the hook.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        // For WH_KEYBOARD_LL, lparam points to a KBDLLHOOKSTRUCT describing the key
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let key_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
        // The Windows key is also swallowed on release: the shell reacts to the key-up
        // (showing the taskbar, opening Start) even when the key-down was blocked
        if is_windows_key(info) || (key_down && should_block(info)) {
            // A non-zero result swallows the key: no application, nor Windows, sees it
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn is_windows_key(info: &KBDLLHOOKSTRUCT) -> bool {
    let key = info.vkCode as u16;
    key == VK_LWIN.0 || key == VK_RWIN.0
}

fn should_block(info: &KBDLLHOOKSTRUCT) -> bool {
    let key = info.vkCode as u16;
    let alt = info.flags.0 & LLKHF_ALTDOWN.0 != 0;
    // The high bit of GetAsyncKeyState is set while the key is held down
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } < 0;

    (alt && (key == VK_TAB.0 || key == VK_F4.0 || key == VK_ESCAPE.0))
        || (ctrl && key == VK_ESCAPE.0)
}
