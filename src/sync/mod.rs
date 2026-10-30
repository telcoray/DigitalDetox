//! Syncing with paired devices on the same network: starting a detox or a Pomodoro on one
//! starts it on the others too.
//!
//! A background thread owns the UDP socket. It broadcasts this device's shared activity every
//! few seconds and on every change, passes on what the other devices announce, and runs the
//! pairing exchange.
//!
//! Windows Firewall often blocks packets that arrive unasked (on a laptop whose network is set
//! as public, or when its prompt was dismissed), but it lets through a direct answer to a
//! broadcast for a few seconds. So whatever a device hears, it answers at once, straight to the
//! sender: the pairing steps, and its own activity in reply to another device's. Decisions (which activity wins, when to lock) are made on the UI thread
//! in main.rs, which talks to the thread through `Sync` and receives `Event`s.

mod config;
mod net;
mod protocol;

use std::collections::HashMap;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use spake2::{Ed25519Group, Spake2};

pub use config::Config;
pub use protocol::{Group, Shared, Version, random_code};
use protocol::{Message, PORT};

/// How often the current activity is broadcast, so devices that wake up or join catch up
const ANNOUNCE_EVERY: Duration = Duration::from_secs(3);
/// How long a pairing code stays valid
const CODE_VALID_FOR: Duration = Duration::from_secs(120);
/// How long the device where the code was typed waits for an answer
const JOIN_TIMEOUT: Duration = Duration::from_secs(15);
/// After answering, how long the inviter waits for the proof that the code was right
const DONE_TIMEOUT: Duration = Duration::from_secs(10);
/// Devices heard from this recently also get every message sent straight to their address
const KNOWN_FOR: Duration = Duration::from_secs(60);

/// Something the other devices said, passed on to the UI thread
#[derive(Clone, Debug)]
pub enum Event {
    /// A paired device announced its activity
    Remote { device: u64, name: String, version: Version, shared: Shared },
    /// Pairing succeeded. `group` is set on the device where the code was typed: it joins it.
    Paired { device: u64, name: String, group: Option<Group> },
    PairingFailed(&'static str),
}

enum Command {
    Publish(Version, Shared),
    SetEnabled(bool),
    SetGroup(Option<Group>),
    Invite(String),
    Join(String),
    CancelPairing,
}

/// Handle to the network thread; it stops when this is dropped
pub struct Sync {
    commands: Sender<Command>,
}

impl Sync {
    pub fn start(
        device: u64,
        name: String,
        group: Option<Group>,
        enabled: bool,
        on_event: impl Fn(Event) + Send + 'static,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", PORT))?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let (commands, receiver) = channel();
        let worker = Worker {
            socket,
            device,
            name,
            group,
            enabled,
            current: None,
            pairing: Pairing::None,
            broadcast: Vec::new(),
            broadcast_refreshed: None,
            known: HashMap::new(),
            last_announce: None,
        };
        std::thread::spawn(move || worker.run(receiver, on_event));
        log!("[sync] listening on UDP port {PORT}");
        Ok(Self { commands })
    }

    pub fn publish(&self, version: Version, shared: Shared) {
        let _ = self.commands.send(Command::Publish(version, shared));
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.commands.send(Command::SetEnabled(enabled));
    }

    pub fn set_group(&self, group: Option<Group>) {
        let _ = self.commands.send(Command::SetGroup(group));
    }

    /// Waits for a device to type `code`, then gives it the group key
    pub fn invite(&self, code: String) {
        let _ = self.commands.send(Command::Invite(code));
    }

    /// Looks for the device showing `code` and joins its group
    pub fn join(&self, code: String) {
        let _ = self.commands.send(Command::Join(code));
    }

    pub fn cancel_pairing(&self) {
        let _ = self.commands.send(Command::CancelPairing);
    }
}

enum Pairing {
    None,
    Inviting {
        code: String,
        until: Instant,
        // Set once a device has answered: a code can be tried only once
        answered: Option<Answered>,
    },
    Joining {
        id: u64,
        state: Spake2<Ed25519Group>,
        hello: Vec<u8>,
        until: Instant,
        last_sent: Instant,
    },
}

struct Answered {
    pairing: u64,
    device: u64,
    name: String,
    reply: Vec<u8>,
    expected_done: Vec<u8>,
    at: Instant,
}

struct Worker {
    socket: UdpSocket,
    device: u64,
    name: String,
    group: Option<Group>,
    enabled: bool,
    current: Option<(Version, Shared)>,
    pairing: Pairing,
    broadcast: Vec<std::net::Ipv4Addr>,
    broadcast_refreshed: Option<Instant>,
    // Addresses of the devices heard from, and when
    known: HashMap<SocketAddr, Instant>,
    last_announce: Option<Instant>,
}

impl Worker {
    fn run(mut self, commands: Receiver<Command>, on_event: impl Fn(Event)) {
        let mut buffer = [0u8; 4096];
        loop {
            loop {
                match commands.try_recv() {
                    Ok(command) => self.command(command),
                    Err(TryRecvError::Empty) => break,
                    // The app is closing
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if let Ok((len, from)) = self.socket.recv_from(&mut buffer) {
                let packet = buffer[..len].to_vec();
                if let Some(event) = self.receive(&packet, from) {
                    on_event(event);
                }
            }
            if let Some(event) = self.housekeeping() {
                on_event(event);
            }
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Publish(version, shared) => {
                self.current = Some((version, shared));
                self.announce();
            }
            Command::SetEnabled(enabled) => {
                self.enabled = enabled;
                self.announce();
            }
            Command::SetGroup(group) => {
                self.group = group;
                self.announce();
            }
            Command::Invite(code) => {
                self.pairing = Pairing::Inviting { code, until: Instant::now() + CODE_VALID_FOR, answered: None };
            }
            Command::Join(code) => {
                let (state, spake) = protocol::joiner_start(&code);
                let id = protocol::random_u64();
                let hello = protocol::encode_pairing(&Message::PairHello {
                    pairing: id,
                    device: self.device,
                    name: self.name.clone(),
                    spake,
                });
                self.send(&hello);
                let now = Instant::now();
                self.pairing = Pairing::Joining { id, state, hello, until: now + JOIN_TIMEOUT, last_sent: now };
            }
            Command::CancelPairing => self.pairing = Pairing::None,
        }
    }

    fn receive(&mut self, packet: &[u8], from: SocketAddr) -> Option<Event> {
        let key = self.group.as_ref().map(|g| g.key);
        match protocol::decode(packet, key.as_ref())? {
            Message::State { group, device, name, version, shared, reply } => {
                let ours = self.group.as_ref().is_some_and(|g| g.id == group);
                if !ours || device == self.device {
                    return None;
                }
                self.known.insert(from, Instant::now());
                if !reply {
                    self.reply_state(from);
                }
                Some(Event::Remote { device, name, version, shared })
            }
            Message::PairHello { pairing, device, name, spake } => {
                if device == self.device {
                    return None;
                }
                self.known.insert(from, Instant::now());
                let Pairing::Inviting { code, answered, .. } = &self.pairing else { return None };
                if let Some(a) = answered {
                    // The joiner didn't hear the answer: say it again. Any other device is
                    // ignored: a code can be tried only once.
                    if a.pairing == pairing {
                        let reply = a.reply.clone();
                        self.send_to(&reply, from);
                        self.send(&reply);
                    }
                    return None;
                }
                let group = self.group.clone()?;
                let answer = protocol::inviter_answer(code, &spake, &group)?;
                let reply = protocol::encode_pairing(&Message::PairReply {
                    pairing,
                    device: self.device,
                    name: self.name.clone(),
                    spake: answer.spake,
                    group: answer.group,
                    proof: answer.proof,
                });
                if let Pairing::Inviting { answered, .. } = &mut self.pairing {
                    *answered = Some(Answered {
                        pairing,
                        device,
                        name,
                        reply: reply.clone(),
                        expected_done: answer.expected_done,
                        at: Instant::now(),
                    });
                }
                self.send_to(&reply, from);
                self.send(&reply);
                None
            }
            reply @ Message::PairReply { .. } => {
                let Message::PairReply { pairing, device, ref name, .. } = reply else { return None };
                if !matches!(&self.pairing, Pairing::Joining { id, .. } if *id == pairing) {
                    return None;
                }
                self.known.insert(from, Instant::now());
                let Pairing::Joining { state, .. } = std::mem::replace(&mut self.pairing, Pairing::None) else {
                    return None;
                };
                match protocol::joiner_finish(state, &reply) {
                    Some((group, proof)) => {
                        let done = protocol::encode_pairing(&Message::PairDone { pairing, proof });
                        // Broadcasts can get lost: a few copies
                        for _ in 0..3 {
                            self.send_to(&done, from);
                            self.send(&done);
                        }
                        self.group = Some(group.clone());
                        self.enabled = true;
                        Some(Event::Paired { device, name: name.clone(), group: Some(group) })
                    }
                    None => Some(Event::PairingFailed("Wrong code. Check it and try again.")),
                }
            }
            Message::PairDone { pairing, proof } => {
                let Pairing::Inviting { answered: Some(a), .. } = &self.pairing else { return None };
                if a.pairing != pairing || !protocol::proofs_match(&proof, &a.expected_done) {
                    return None;
                }
                let event = Event::Paired { device: a.device, name: a.name.clone(), group: None };
                self.pairing = Pairing::None;
                self.enabled = true;
                self.announce();
                Some(event)
            }
        }
    }

    /// Timeouts, resending, and the regular announcement
    fn housekeeping(&mut self) -> Option<Event> {
        let now = Instant::now();
        let mut event = None;
        match &mut self.pairing {
            Pairing::None => {}
            Pairing::Inviting { until, answered, .. } => {
                if answered.as_ref().is_some_and(|a| now - a.at > DONE_TIMEOUT) {
                    self.pairing = Pairing::None;
                    event = Some(Event::PairingFailed("The other device entered a different code."));
                } else if now > *until && answered.is_none() {
                    self.pairing = Pairing::None;
                    event = Some(Event::PairingFailed("The code has expired."));
                }
            }
            Pairing::Joining { hello, until, last_sent, .. } => {
                if now > *until {
                    self.pairing = Pairing::None;
                    event = Some(Event::PairingFailed(
                        "No device answered. Is it showing the code, on the same network?",
                    ));
                } else if now - *last_sent >= Duration::from_secs(1) {
                    *last_sent = now;
                    let hello = hello.clone();
                    self.send(&hello);
                }
            }
        }
        if self.last_announce.is_none_or(|t| now - t >= ANNOUNCE_EVERY) {
            self.announce();
        }
        event
    }

    fn announce(&mut self) {
        self.last_announce = Some(Instant::now());
        if let Some(packet) = self.state_packet(false) {
            self.send(&packet);
        }
    }

    /// Answers another device's announcement with this one's activity, straight to it
    fn reply_state(&mut self, to: SocketAddr) {
        if let Some(packet) = self.state_packet(true) {
            self.send_to(&packet, to);
        }
    }

    fn state_packet(&self, reply: bool) -> Option<Vec<u8>> {
        let (Some(group), Some((version, shared)), true) = (&self.group, self.current, self.enabled) else {
            return None;
        };
        let message =
            Message::State { group: group.id, device: self.device, name: self.name.clone(), version, shared, reply };
        Some(protocol::encode_signed(&message, &group.key))
    }

    fn send_to(&self, packet: &[u8], to: SocketAddr) {
        let _ = self.socket.send_to(packet, to);
    }

    /// To every local network, and straight to the devices heard from recently
    fn send(&mut self, packet: &[u8]) {
        self.known.retain(|_, seen| seen.elapsed() < KNOWN_FOR);
        for &address in self.known.keys() {
            let _ = self.socket.send_to(packet, address);
        }
        // Networks come and go (Wi-Fi reconnecting, VPN): look again from time to time
        if self.broadcast_refreshed.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            self.broadcast = net::broadcast_addresses();
            self.broadcast_refreshed = Some(Instant::now());
        }
        for address in &self.broadcast {
            let _ = self.socket.send_to(packet, SocketAddrV4::new(*address, PORT));
        }
    }
}
