fn main() {
    let root = std::path::Path::new("../../src/sync");
    println!("cargo:rerun-if-changed=../../src/sync/mod.rs");
    println!("cargo:rerun-if-changed=../../src/sync/protocol.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // Compile the desktop worker unchanged except its platform adapters. No fork of the wire protocol.
    let worker = std::fs::read_to_string(root.join("mod.rs")).unwrap()
        .replace("//!", "//").replace("mod config;", "")
        .replace("pub use config::Config;", "")
        .replace("mod net;", "mod net { pub fn broadcast_addresses() -> Vec<std::net::Ipv4Addr> { crate::broadcasts() } }");
    std::fs::write(out.join("sync.rs"), worker).unwrap();
    std::fs::copy(root.join("protocol.rs"), out.join("protocol.rs")).unwrap();
}
