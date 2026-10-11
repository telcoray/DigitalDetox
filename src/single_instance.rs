//! Allows only one copy of the app at a time, like the named Mutex of the C# version.
//!
//! A named mutex is a Windows object that exists once per user session: the first copy of the
//! app creates it, and any later copy finds it already there.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
use windows::core::w;

/// Holds the mutex for as long as the app runs (it's released when this value is dropped).
/// Unlike the C# version, Rust guarantees it can't be released early by a garbage collector.
pub struct SingleInstance(Option<HANDLE>);

/// Another copy of the app is already running.
pub struct AlreadyRunning;

impl SingleInstance {
    pub fn acquire() -> Result<Self, AlreadyRunning> {
        unsafe {
            // Separate name from the C# version, so the two versions don't block each other
            let Ok(handle) = CreateMutexW(None, true, w!("DigitalDetoxRust")) else {
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

/// Same message as the C# version, in a standard Windows message box
pub fn show_already_running() {
    unsafe {
        MessageBoxW(
            None,
            w!("Digital Detox is already running."),
            w!("Already Running"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
