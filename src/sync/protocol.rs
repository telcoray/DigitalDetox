//! What paired devices say to each other, and how it's protected.
//!
//! Everything travels as UDP broadcasts on the local network, so devices find each other
//! without knowing their addresses. Every message is JSON with a one-byte prefix:
//! - `S`: the shared activity, signed with the group key (HMAC-SHA256). Devices that aren't
//!   paired can't read anything useful from it, nor forge one to lock your screen.
//! - `P`: pairing. The 6-digit code is never sent: both sides run SPAKE2 with it, which gives
//!   them the same secret key only if they typed the same code, and an eavesdropper can't try
//!   codes offline. The inviter then sends the group key encrypted with that secret.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};

type HmacSha256 = Hmac<Sha256>;

/// UDP port used by Digital Detox on the local network
pub const PORT: u16 = 47821;
const TAG_LEN: usize = 32;

const ID_INVITER: &[u8] = b"digital-detox inviter";
const ID_JOINER: &[u8] = b"digital-detox joiner";

/// The activity shared by paired devices. Times are Unix times in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Shared {
    Idle,
    Detox { ends_at: u64 },
    Pomodoro { start: u64 },
}

/// When a change was made and by which device: on every device the most recent change wins
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Version {
    pub time: u64,
    pub device: u64,
}

/// The secret shared by all paired devices
#[derive(Clone, PartialEq)]
pub struct Group {
    pub id: u64,
    pub key: [u8; 32],
}

// Never print the key, not even in the log
impl std::fmt::Debug for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Group({:016x})", self.id)
    }
}

impl Group {
    pub fn random() -> Self {
        let mut key = [0u8; 32];
        fill_random(&mut key);
        Self { id: random_u64(), key }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// Sent every few seconds and on every change. A device that hears one answers straight
    /// to the sender with its own (`reply`), see `Worker::receive`.
    State {
        group: u64,
        device: u64,
        name: String,
        version: Version,
        shared: Shared,
        #[serde(default)]
        reply: bool,
    },
    /// Pairing, step 1 (the device where the code was typed): its half of the key exchange
    PairHello { pairing: u64, device: u64, name: String, spake: Vec<u8> },
    /// Step 2 (the device showing the code): its half, and the group encrypted with the new key
    PairReply { pairing: u64, device: u64, name: String, spake: Vec<u8>, group: Vec<u8>, proof: Vec<u8> },
    /// Step 3: proof that the code was right, so the inviter can add the device to its list
    PairDone { pairing: u64, proof: Vec<u8> },
}

pub fn encode_signed(message: &Message, key: &[u8; 32]) -> Vec<u8> {
    let mut packet = vec![b'S'];
    packet.extend(serde_json::to_vec(message).unwrap_or_default());
    let tag = mac(key, &[&packet]);
    packet.extend(tag);
    packet
}

pub fn encode_pairing(message: &Message) -> Vec<u8> {
    let mut packet = vec![b'P'];
    packet.extend(serde_json::to_vec(message).unwrap_or_default());
    packet
}

/// Reads a packet. Signed ones are only accepted with a valid signature for `key`.
pub fn decode(packet: &[u8], key: Option<&[u8; 32]>) -> Option<Message> {
    match packet.first()? {
        b'S' => {
            let key = key?;
            let body = packet.len().checked_sub(TAG_LEN)?;
            let (signed, tag) = packet.split_at(body);
            let mut check = HmacSha256::new_from_slice(key).ok()?;
            check.update(signed);
            check.verify_slice(tag).ok()?;
            let message: Message = serde_json::from_slice(&signed[1..]).ok()?;
            matches!(message, Message::State { .. }).then_some(message)
        }
        b'P' => {
            let message: Message = serde_json::from_slice(&packet[1..]).ok()?;
            (!matches!(message, Message::State { .. })).then_some(message)
        }
        _ => None,
    }
}

/// The device where the code is typed starts the exchange
pub fn joiner_start(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    Spake2::<Ed25519Group>::start_b(&Password::new(code), &Identity::new(ID_INVITER), &Identity::new(ID_JOINER))
}

/// The inviter's answer to a PairHello
pub struct Answer {
    pub spake: Vec<u8>,
    pub group: Vec<u8>,
    pub proof: Vec<u8>,
    /// What the joiner's PairDone must contain if it had the same code
    pub expected_done: Vec<u8>,
}

/// The device showing the code answers the joiner, sending `group` encrypted
pub fn inviter_answer(code: &str, joiner_spake: &[u8], group: &Group) -> Option<Answer> {
    let (state, spake) =
        Spake2::<Ed25519Group>::start_a(&Password::new(code), &Identity::new(ID_INVITER), &Identity::new(ID_JOINER));
    let secret = state.finish(joiner_spake).ok()?;
    let encrypted = xor_group(&secret, group);
    Some(Answer {
        spake,
        proof: mac(&secret, &[b"inviter", &encrypted]),
        group: encrypted,
        expected_done: mac(&secret, &[b"joiner"]),
    })
}

/// The joiner checks the answer: with the wrong code, the proof doesn't match.
/// Returns the group and the proof for PairDone.
pub fn joiner_finish(state: Spake2<Ed25519Group>, reply: &Message) -> Option<(Group, Vec<u8>)> {
    let Message::PairReply { spake, group, proof, .. } = reply else { return None };
    let secret = state.finish(spake).ok()?;
    let mut check = HmacSha256::new_from_slice(&secret).ok()?;
    check.update(b"inviter");
    check.update(group);
    check.verify_slice(proof).ok()?;
    let group = unxor_group(&secret, group)?;
    Some((group, mac(&secret, &[b"joiner"])))
}

/// Constant-time comparison of two proofs
pub fn proofs_match(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn mac(key: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().to_vec()
}

/// The group id and key, XORed with pads derived from the exchanged secret (used only once)
fn xor_group(secret: &[u8], group: &Group) -> Vec<u8> {
    let mut plain = group.id.to_le_bytes().to_vec();
    plain.extend(group.key);
    let mut pad = mac(secret, &[b"group pad 1"]);
    pad.extend(mac(secret, &[b"group pad 2"]));
    plain.iter().zip(pad).map(|(p, k)| p ^ k).collect()
}

fn unxor_group(secret: &[u8], encrypted: &[u8]) -> Option<Group> {
    if encrypted.len() != 40 {
        return None;
    }
    let mut pad = mac(secret, &[b"group pad 1"]);
    pad.extend(mac(secret, &[b"group pad 2"]));
    let plain: Vec<u8> = encrypted.iter().zip(pad).map(|(c, k)| c ^ k).collect();
    Some(Group { id: u64::from_le_bytes(plain[..8].try_into().ok()?), key: plain[8..].try_into().ok()? })
}

pub fn fill_random(buffer: &mut [u8]) {
    if let Err(e) = getrandom::getrandom(buffer) {
        // Windows' random generator doesn't fail in practice
        panic!("no random numbers available: {e}");
    }
}

pub fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    fill_random(&mut bytes);
    u64::from_le_bytes(bytes)
}

/// A 6-digit pairing code, e.g. "048213"
pub fn random_code() -> String {
    format!("{:06}", random_u64() % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_message() -> Message {
        Message::State {
            group: 7,
            device: 1,
            name: "PC".into(),
            version: Version { time: 10, device: 1 },
            shared: Shared::Pomodoro { start: 123 },
            reply: false,
        }
    }

    #[test]
    fn signed_messages_need_the_right_key() {
        let key = [1u8; 32];
        let packet = encode_signed(&state_message(), &key);
        assert_eq!(decode(&packet, Some(&key)), Some(state_message()));
        assert_eq!(decode(&packet, Some(&[2u8; 32])), None);
        assert_eq!(decode(&packet, None), None);
        let mut tampered = packet.clone();
        tampered[5] ^= 1;
        assert_eq!(decode(&tampered, Some(&key)), None);
        // A state message can't sneak in unsigned
        assert_eq!(decode(&encode_pairing(&state_message()), Some(&key)), None);
    }

    fn pair(inviter_code: &str, joiner_code: &str) -> Option<Group> {
        let group = Group::random();
        let (joiner, hello) = joiner_start(joiner_code);
        let answer = inviter_answer(inviter_code, &hello, &group)?;
        let reply = Message::PairReply {
            pairing: 1,
            device: 2,
            name: "Laptop".into(),
            spake: answer.spake,
            group: answer.group,
            proof: answer.proof,
        };
        let (received, done) = joiner_finish(joiner, &reply)?;
        assert!(proofs_match(&done, &answer.expected_done));
        assert_eq!(received, group);
        Some(received)
    }

    #[test]
    fn pairing_works_only_with_the_same_code() {
        assert!(pair("123456", "123456").is_some());
        assert!(pair("123456", "123457").is_none());
    }

    #[test]
    fn versions_order_by_time_then_device() {
        assert!(Version { time: 2, device: 1 } > Version { time: 1, device: 9 });
        assert!(Version { time: 2, device: 2 } > Version { time: 2, device: 1 });
    }
}
