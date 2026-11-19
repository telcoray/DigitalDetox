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
