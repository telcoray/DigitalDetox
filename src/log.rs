//! Diagnostic messages: printed to the console (visible with `cargo run`) and also written to
//! %LOCALAPPDATA%\DigitalDetox\digital-detox.log, so they're available with the released
//! executable too, which has no console. The file only keeps the latest run.

use std::io::Write;
use std::path::PathBuf;

/// The app's own folder for small files: %LOCALAPPDATA%\DigitalDetox
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("DigitalDetox")
}

fn log_path() -> PathBuf {
    data_dir().join("digital-detox.log")
}

/// Starts a fresh log file for this run
pub fn start() {
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(log_path(), format!("Digital Detox {}\n", env!("CARGO_PKG_VERSION")));
}

pub fn write(message: &str) {
    eprintln!("{message}");
    let line = format!("{} {message}\n", chrono::Local::now().format("%H:%M:%S"));
    if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open(log_path()) {
        let _ = file.write_all(line.as_bytes());
    }
}
