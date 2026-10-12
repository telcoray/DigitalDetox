//! Allows only one copy of the app at a time. Starting it again just brings back the window
//! of the copy that's already running (which may be sitting in the notification area).
//!
//! Two named Windows objects, which exist once per user session:
//! - a mutex: the first copy creates it, any later copy finds it already there;
//! - an event: the "doorbell" a later copy rings to ask the running one to show its window.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
use windows::core::{PCWSTR, w};

// Separate names from the C# version, so the two versions don't interfere with each other
const MUTEX_NAME: PCWSTR = w!("DigitalDetoxRust");
const SHOW_EVENT_NAME: PCWSTR = w!("DigitalDetoxRust-Show");

/// Holds the mutex for as long as the app runs (it's released when this value is dropped).
/// Unlike the C# version, Rust guarantees it can't be released early by a garbage collector.
pub struct SingleInstance(Option<HANDLE>);

/// Another copy of the app is already running.
pub struct AlreadyRunning;

impl SingleInstance {
    pub fn acquire() -> Result<Self, AlreadyRunning> {
        unsafe {
            let Ok(handle) = CreateMutexW(None, true, MUTEX_NAME) else {
                // Couldn't create the mutex at all: better to run than to refuse to start
                return Ok(Self(None));
            };
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Err(AlreadyRunning);
            }
            Ok(Self(Some(handle)))
        }
    }

    /// Calls `on_show` every time another copy of the app is started.
    /// `on_show` runs on a background thread.
    pub fn listen_for_show_requests(&self, on_show: impl Fn() + Send + 'static) {
        // Auto-reset event: each ring wakes the listener once
        let Ok(event) = (unsafe { CreateEventW(None, false, false, SHOW_EVENT_NAME) }) else { return };
        // HANDLE can't cross threads as is; its raw value can
        let raw = event.0 as usize;
        std::thread::spawn(move || {
            let event = HANDLE(raw as *mut _);
            // Lives as long as the app: the thread ends when the process does
            while unsafe { WaitForSingleObject(event, INFINITE) }.0 == 0 {
                on_show();
            }
        });
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if let Some(handle) = self.0 {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Called by a second copy: asks the running one to show its window. Falls back to the old
/// message if that's not possible (e.g. the running copy is an older version of the app).
pub fn show_running_instance() {
    unsafe {
        if let Ok(event) = OpenEventW(EVENT_MODIFY_STATE, false, SHOW_EVENT_NAME) {
            let rang = SetEvent(event).is_ok();
            let _ = CloseHandle(event);
            if rang {
                return;
            }
        }
        MessageBoxW(None, w!("Digital Detox is already running."), w!("Already Running"), MB_OK | MB_ICONINFORMATION);
    }
}
