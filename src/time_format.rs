//! Formats times the way the user's Windows settings say (like .NET's "T"/"t" formats in the
//! C# version): 19:19:33 with Italian settings, 7:19:33 PM with US settings.

use chrono::Timelike;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::{GetTimeFormatEx, TIME_FORMAT_FLAGS, TIME_NOSECONDS};
use windows::core::PCWSTR;

/// With seconds, e.g. "19:19:33" (lock screen)
pub fn long_time(time: chrono::DateTime<chrono::Local>) -> String {
    format(time, TIME_FORMAT_FLAGS(0), "%H:%M:%S")
}

/// Without seconds, e.g. "19:19" (main window preview)
pub fn short_time(time: chrono::DateTime<chrono::Local>) -> String {
    format(time, TIME_NOSECONDS, "%H:%M")
}

fn format(time: chrono::DateTime<chrono::Local>, flags: TIME_FORMAT_FLAGS, fallback: &str) -> String {
    // Only the time fields matter: GetTimeFormatEx ignores the date
    let system_time = SYSTEMTIME {
        wHour: time.hour() as u16,
        wMinute: time.minute() as u16,
        wSecond: time.second() as u16,
        ..Default::default()
    };
    let mut buffer = [0u16; 64];
    // Null locale and format = the current user's locale and its default time format
    let len = unsafe { GetTimeFormatEx(PCWSTR::null(), flags, Some(&system_time), PCWSTR::null(), Some(&mut buffer)) };
    if len > 1 {
        // `len` counts the terminating zero
        String::from_utf16_lossy(&buffer[..len as usize - 1])
    } else {
        time.format(fallback).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> chrono::DateTime<chrono::Local> {
        chrono::Local.with_ymd_and_hms(2026, 10, 11, 19, 19, 33).unwrap()
    }

    #[test]
    fn long_time_has_seconds() {
        let text = long_time(sample());
        println!("long: {text}");
        // 24-hour ("19:19:33") or 12-hour ("7:19:33 PM") depending on the PC's settings
        assert!(text.contains("19:33"), "unexpected format: {text}");
    }

    #[test]
    fn short_time_has_no_seconds() {
        let text = short_time(sample());
        println!("short: {text}");
        assert!(text.contains("19") && !text.contains("33"), "unexpected format: {text}");
    }
}
