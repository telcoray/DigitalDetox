//! Laptop screen brightness: lowered to the minimum during a detox, restored afterwards.
//!
//! Uses WMI (the same interface Windows' own brightness slider uses), which only exists for
//! built-in laptop panels: on a desktop with external monitors every call quietly does nothing.
//!
//! WMI calls can take a moment, so they run on a dedicated background thread that handles one
//! command at a time, in order. The original brightness is saved to a file before lowering it,
//! so it can be restored even if the app is closed abruptly (see `recover`).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wmi::WMIConnection;

/// Brightness during the detox, in percent
const DETOX_BRIGHTNESS: u8 = 0;
/// The change happens in small steps, so it feels like a fade rather than a jump
const FADE_STEPS: u8 = 10;
const LOWER_DURATION: Duration = Duration::from_millis(2500);
const RESTORE_DURATION: Duration = Duration::from_millis(800);

enum Command {
    Lower,
    Restore,
}

/// Fades the brightness down to the minimum, remembering the current value
pub fn lower() {
    send(Command::Lower);
}

/// Fades the brightness back to what it was before `lower`
pub fn restore() {
    send(Command::Restore);
}

/// At start-up: if a previous run ended without restoring the brightness, restore it now
pub fn recover() {
    if read_backup().is_some() {
        send(Command::Restore);
    }
}

fn send(command: Command) {
    // The worker thread is started the first time it's needed
    static WORKER: OnceLock<Sender<Command>> = OnceLock::new();
    let sender = WORKER.get_or_init(|| {
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            // A WMI connection belongs to the thread that created it, so it's created here
            let wmi = match WMIConnection::with_namespace_path("ROOT\\WMI") {
                Ok(wmi) => wmi,
                Err(e) => return eprintln!("Brightness control unavailable: {e}"),
            };
            for command in receiver {
                match command {
                    Command::Lower => lower_now(&wmi),
                    Command::Restore => restore_now(&wmi),
                }
            }
        });
        sender
    });
    let _ = sender.send(command);
}

fn lower_now(wmi: &WMIConnection) {
    let Some(current) = current_brightness(wmi) else { return };
    // Keep an existing backup: it holds the real original value if a previous run never restored it
    if read_backup().is_none() {
        write_backup(current);
    }
    fade(wmi, current, DETOX_BRIGHTNESS, LOWER_DURATION);
}

fn restore_now(wmi: &WMIConnection) {
    let Some(original) = read_backup() else { return };
    let current = current_brightness(wmi).unwrap_or(DETOX_BRIGHTNESS);
    fade(wmi, current, original, RESTORE_DURATION);
    let _ = std::fs::remove_file(backup_path());
}

fn fade(wmi: &WMIConnection, from: u8, to: u8, duration: Duration) {
    for step in 1..=FADE_STEPS {
        let value = from as i32 + (to as i32 - from as i32) * step as i32 / FADE_STEPS as i32;
        set_brightness(wmi, value as u8);
        std::thread::sleep(duration / FADE_STEPS as u32);
    }
}

// WMI classes, mirrored as Rust structs (field names must match WMI's exactly)

#[derive(Deserialize)]
#[allow(non_snake_case, non_camel_case_types)]
struct WmiMonitorBrightness {
    CurrentBrightness: u8,
}

#[derive(Deserialize)]
#[allow(non_snake_case, non_camel_case_types)]
struct WmiMonitorBrightnessMethods {
    __Path: String,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct WmiSetBrightness {
    Timeout: u32,
    Brightness: u8,
}

fn current_brightness(wmi: &WMIConnection) -> Option<u8> {
    let screens: Vec<WmiMonitorBrightness> = wmi.query().ok()?;
    screens.first().map(|s| s.CurrentBrightness)
}

fn set_brightness(wmi: &WMIConnection, value: u8) {
    let Ok(screens) = wmi.query::<WmiMonitorBrightnessMethods>() else { return };
    for screen in screens {
        let input = WmiSetBrightness { Timeout: 0, Brightness: value };
        let _: Result<(), _> = wmi.exec_instance_method::<WmiMonitorBrightnessMethods, ()>(
            &screen.__Path,
            "WmiSetBrightness",
            input,
        );
    }
}

// Backup of the original brightness, in %LOCALAPPDATA%\DigitalDetox

fn backup_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("DigitalDetox").join("brightness-backup.txt")
}

fn read_backup() -> Option<u8> {
    std::fs::read_to_string(backup_path()).ok()?.trim().parse().ok()
}

fn write_backup(value: u8) {
    let path = backup_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, value.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only reads the brightness, never changes it. Prints None on PCs without a built-in panel.
    #[test]
    fn reads_current_brightness() {
        let wmi = WMIConnection::with_namespace_path("ROOT\\WMI").expect("WMI connection");
        println!("current brightness: {:?}", current_brightness(&wmi));
        let methods: Vec<WmiMonitorBrightnessMethods> = wmi.query().unwrap_or_default();
        println!("screens with brightness control: {}", methods.len());
    }
}
