//! The app's own preferences, in %LOCALAPPDATA%\DigitalDetox\settings.json. "Start with
//! Windows" lives in the registry instead (see autostart.rs), and syncing in sync.json.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Start the Pomodoro by itself on the first use of the PC each day (see auto_pomodoro.rs)
    #[serde(default)]
    pub auto_pomodoro: bool,
    /// The last day it was offered or already running, as "2026-11-19"
    #[serde(default)]
    pub auto_pomodoro_day: String,
    /// Offer a break after a long stretch at the PC (see break_reminder.rs)
    #[serde(default)]
    pub break_reminder: bool,
    /// Detoxes at set times each day, in time order (see schedule.rs)
    #[serde(default)]
    pub schedule: Vec<Appointment>,
}

/// A detox at a set time on some days of the week: at 13:00 for 30 minutes, Monday to Friday
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Appointment {
    pub hour: u32,
    pub minute: u32,
    pub minutes: u32,
    /// One bit per day, Monday first (bit 0) to Sunday (bit 6). Set times saved before days
    /// existed are for every day.
    #[serde(default = "every_day")]
    pub days: u8,
}

pub const EVERY_DAY: u8 = 0b111_1111;

fn every_day() -> u8 {
    EVERY_DAY
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read(path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::log::data_dir());
        if let Ok(json) = serde_json::to_vec_pretty(self)
            && let Err(e) = std::fs::write(path(), json)
        {
            log!("[settings] couldn't save the settings: {e}");
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::log::data_dir().join("settings.json")
}
