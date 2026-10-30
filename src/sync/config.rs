//! What this device remembers about syncing, in %LOCALAPPDATA%\DigitalDetox\sync.json.
//! The group key is encrypted with DPAPI, so only this Windows account can read it back.

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData};

use super::protocol::{self, Group};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Random id of this device, created the first time
    pub device: u64,
    /// "Sync with paired devices"
    pub enabled: bool,
    group_id: Option<u64>,
    /// The group key encrypted with DPAPI, in hex
    group_key: Option<String>,
    /// The other devices seen in the group, for the list in the Sync tab
    pub peers: Vec<Peer>,
}

impl Config {
    pub fn load() -> Self {
        let mut config: Config = std::fs::read(path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if config.device == 0 {
            config.device = protocol::random_u64();
            config.save();
        }
        config
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::log::data_dir());
        if let Ok(json) = serde_json::to_vec_pretty(self)
            && let Err(e) = std::fs::write(path(), json)
        {
            log!("[sync] couldn't save the settings: {e}");
        }
    }

    pub fn group(&self) -> Option<Group> {
        let id = self.group_id?;
        let encrypted = from_hex(self.group_key.as_deref()?)?;
        let key = unprotect(&encrypted)?.try_into().ok()?;
        Some(Group { id, key })
    }

    pub fn set_group(&mut self, group: Option<&Group>) {
        self.group_id = group.map(|g| g.id);
        self.group_key = group.and_then(|g| protect(&g.key)).map(|bytes| to_hex(&bytes));
        self.peers.clear();
    }

    /// Adds the device to the list, or updates its name. Returns true if anything changed.
    pub fn remember_peer(&mut self, id: u64, name: &str) -> bool {
        match self.peers.iter_mut().find(|p| p.id == id) {
            Some(peer) if peer.name == name => false,
            Some(peer) => {
                peer.name = name.to_owned();
                true
            }
            None => {
                self.peers.push(Peer { id, name: name.to_owned() });
                true
            }
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::log::data_dir().join("sync.json")
}

fn protect(data: &[u8]) -> Option<Vec<u8>> {
    crypt(data, true)
}

fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    crypt(data, false)
}

fn crypt(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let result = unsafe {
        if protect {
            CryptProtectData(&input, None, None, None, None, 0, &mut output)
        } else {
            CryptUnprotectData(&input, None, None, None, None, 0, &mut output)
        }
    };
    if result.is_err() {
        log!("[sync] DPAPI failed: {result:?}");
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
    }
    Some(bytes)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_key_survives_a_round_trip() {
        let group = Group::random();
        let mut config = Config::default();
        config.set_group(Some(&group));
        assert_ne!(config.group_key.as_deref(), Some(to_hex(&group.key).as_str()));
        assert_eq!(config.group(), Some(group));
    }
}
