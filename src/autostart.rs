//! Start with Windows: an entry under HKCU\...\CurrentVersion\Run starts the app at sign-in,
//! hidden in the system tray (`--background`). It's per user: no administrator rights needed.
//!
//! Windows keeps its own on/off switch for these entries (Settings → Apps → Startup, Task
//! Manager → Startup apps) under StartupApproved. An entry turned off there doesn't run, so it
//! counts as off here too, and turning it on from the tray menu clears that switch.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{PCWSTR, w};

/// Command-line flag: start hidden in the system tray, without opening the main window
pub const BACKGROUND_FLAG: &str = "--background";

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
const VALUE_NAME: PCWSTR = w!("DigitalDetox");

pub fn is_enabled() -> bool {
    let registered =
        unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME, RRF_RT_REG_SZ, None, None, None) } == ERROR_SUCCESS;
    registered && !turned_off_in_windows()
}

pub fn set_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    if enabled {
        // The path of the running executable, quoted in case it contains spaces
        let command = format!("\"{}\" {BACKGROUND_FLAG}", std::env::current_exe()?.display());
        // Registry strings are UTF-16 and end with a 0
        let data: Vec<u16> = command.encode_utf16().chain([0]).collect();
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                VALUE_NAME,
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * size_of::<u16>()) as u32,
            )
        }
        .ok()?;
    } else {
        ignore_missing(unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME) }).ok()?;
    }
    // Either way Windows' own switch goes: when turning on it would keep the entry from running,
    // when turning off it's no longer needed
    let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, APPROVED_KEY, VALUE_NAME) };
    Ok(())
}

/// True when the entry was turned off in Windows' Startup settings
fn turned_off_in_windows() -> bool {
    // 12 bytes: the first one is even (2, 6) when allowed, odd (3, 7) when turned off
    let mut data = [0u8; 12];
    let mut size = data.len() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            APPROVED_KEY,
            VALUE_NAME,
            RRF_RT_REG_BINARY,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    result == ERROR_SUCCESS && data[0] & 1 == 1
}

/// Deleting a value that isn't there is fine: the result is the same
fn ignore_missing(result: WIN32_ERROR) -> WIN32_ERROR {
    if result == ERROR_FILE_NOT_FOUND { ERROR_SUCCESS } else { result }
}
