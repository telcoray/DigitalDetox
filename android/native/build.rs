//! Copies the Sync worker of the Windows app (`src/sync`) for the Android build, so the protocol
//! is never forked (`mod.rs`, `protocol.rs` and `clock.rs`). Only the Windows-specific modules
//! are swapped out: `config` (Java saves the
//! settings) and `net` (Java finds the broadcast addresses, see `broadcasts` in lib.rs).

use std::path::{Path, PathBuf};

/// (text in mod.rs, what replaces it)
const REPLACEMENTS: [(&str, &str); 4] = [
    // Inner doc comments aren't allowed in the middle of lib.rs, where the file is included
    ("//!", "//"),
    ("mod config;", ""),
    ("pub use config::Config;", ""),
    (
        "mod net;",
        "mod net { pub fn broadcast_addresses() -> Vec<std::net::Ipv4Addr> { crate::broadcasts() } }",
    ),
];

fn main() {
    let root = Path::new("../../src/sync");
    println!("cargo:rerun-if-changed=../../src/sync/mod.rs");
    println!("cargo:rerun-if-changed=../../src/sync/protocol.rs");
    println!("cargo:rerun-if-changed=../../src/sync/clock.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut worker = std::fs::read_to_string(root.join("mod.rs")).unwrap();
    for (from, to) in REPLACEMENTS {
        // If mod.rs changed and a line isn't there any more, stop here rather than build
        // something that differs from Windows in a way nobody notices
        assert!(worker.contains(from), "src/sync/mod.rs no longer contains {from:?}: update android/native/build.rs");
        worker = worker.replace(from, to);
    }
    std::fs::write(out.join("sync.rs"), worker).unwrap();
    std::fs::copy(root.join("protocol.rs"), out.join("protocol.rs")).unwrap();
    std::fs::copy(root.join("clock.rs"), out.join("clock.rs")).unwrap();
}
