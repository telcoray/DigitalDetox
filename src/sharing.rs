//! The app's side of syncing with paired devices (the network side is in sync/): what to tell
//! them when something starts here, and what to do when one of them starts something.
//!
//! Every change carries a `Version` (when, and on which device): the most recent one wins
//! everywhere. A change that arrives while this device is locked waits until the lock ends,
//! so another device can never unlock this one early; the emergency exit, likewise, only
//! unlocks the device it's used on.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ModelRc, SharedString, VecModel};

use crate::session::SessionKind;
use crate::sync::{self, Config, Event, Group, Shared, Version};
use crate::{Action, App, SyncDevice, time_format};

/// A device counts as online if it was heard from this recently (it announces every 3 s)
const ONLINE_FOR: Duration = Duration::from_secs(10);
/// A detox with less than this left isn't worth locking for
const MIN_DETOX: Duration = Duration::from_secs(5);

/// What the Sync tab shows, see `pairing-mode` in main_window.slint
#[derive(Clone, Copy, PartialEq)]
enum Pairing {
    None = 0,
    ShowingCode = 1,
    TypingCode = 2,
    Joining = 3,
}

pub struct Sharing {
    config: RefCell<Config>,
    // The network thread, started once this device is paired or starts pairing
    net: RefCell<Option<sync::Sync>>,
    // The activity this device follows, and when it was decided
    current: Cell<(Version, Shared)>,
    // A paired device changed the activity while this one was locked
    apply_after_lock: Cell<bool>,
    // When each paired device was last heard from
    seen: RefCell<HashMap<u64, Instant>>,
    pairing: Cell<Pairing>,
}

impl Sharing {
    pub fn load() -> Self {
        Self {
            config: RefCell::new(Config::load()),
            net: RefCell::default(),
            current: Cell::new((Version::default(), Shared::Idle)),
            apply_after_lock: Cell::new(false),
            seen: RefCell::default(),
            pairing: Cell::new(Pairing::None),
        }
    }
}

/// Connects the Sync tab, and starts listening if this device is already paired
pub fn setup(app: &Rc<App>) {
    let window = &app.window;
    window.on_sync_toggled(|| crate::with_app(toggle));
    window.on_pair_clicked(|| crate::with_app(invite));
    window.on_enter_code_clicked(|| {
        crate::with_app(|app| {
            app.window.set_code_text(SharedString::new());
            set_pairing(app, Pairing::TypingCode, "");
        })
    });
    window.on_join_code(|code| crate::with_app(|app| join(app, &code)));
    window.on_cancel_pairing(|| {
        crate::with_app(|app| {
            if let Some(net) = app.sharing.net.borrow().as_ref() {
                net.cancel_pairing();
            }
            set_pairing(app, Pairing::None, "");
        })
    });
    window.on_unpair_clicked(|| crate::with_app(unpair));

    if app.sharing.config.borrow().group().is_some() {
        start_net(app);
    }
    update_view(app);
}

/// Something started or stopped on this device: paired devices follow
pub fn publish(app: &App, shared: Shared) {
    let device = app.sharing.config.borrow().device;
    let version = Version { time: time_format::now_ms(), device };
    app.sharing.current.set((version, shared));
    if let Some(net) = app.sharing.net.borrow().as_ref() {
        net.publish(version, shared);
    }
}

pub fn on_event(app: &Rc<App>, event: Event) {
    let sharing = &app.sharing;
    match event {
        Event::Remote { device, name, version, shared } => {
            sharing.seen.borrow_mut().insert(device, Instant::now());
            let mut config = sharing.config.borrow_mut();
            if config.remember_peer(device, &name) {
                config.save();
            }
            let enabled = config.enabled;
            drop(config);
            if enabled && version > sharing.current.get().0 {
                log!("[sync] following {name}: {shared:?}");
                sharing.current.set((version, shared));
                // Passed on as is, so a device that only hears this one catches up too
                if let Some(net) = sharing.net.borrow().as_ref() {
                    net.publish(version, shared);
                }
                if app.session.borrow().is_some() {
                    sharing.apply_after_lock.set(true);
                } else {
                    apply(app);
                }
            }
        }
        Event::Paired { device, name, group } => {
            log!("[sync] paired with {name}");
            let mut config = sharing.config.borrow_mut();
            if let Some(group) = &group {
                // The device where the code was typed joins the other's group
                config.set_group(Some(group));
            }
            config.remember_peer(device, &name);
            config.enabled = true;
            config.save();
            drop(config);
            sharing.seen.borrow_mut().insert(device, Instant::now());
            if let Some(net) = sharing.net.borrow().as_ref() {
                if group.is_some() {
                    net.set_group(group);
                }
                net.set_enabled(true);
            }
            set_pairing(app, Pairing::None, &format!("Paired with {name}"));
        }
        Event::PairingFailed(reason) => {
            log!("[sync] pairing failed: {reason}");
            // Where the code was typed, let it be typed again
            let next = if sharing.pairing.get() == Pairing::Joining { Pairing::TypingCode } else { Pairing::None };
            set_pairing(app, next, reason);
        }
    }
    update_view(app);
}

/// The lock screen has closed: catch up with what paired devices did meanwhile
pub fn after_lock(app: &Rc<App>) {
    if app.sharing.apply_after_lock.replace(false) {
        apply(app);
    }
}

/// Makes this device do what `current` says
fn apply(app: &Rc<App>) {
    match app.sharing.current.get().1 {
        Shared::Idle => crate::stop_pomodoro(app),
        Shared::Pomodoro { start } => {
            let running = app.pomodoro.borrow().as_ref().map(|run| run.tracker.start_ms());
            if running != Some(start) {
                crate::run_pomodoro(app, start);
            }
        }
        Shared::Detox { ends_at } => {
            let left = Duration::from_millis(ends_at.saturating_sub(time_format::now_ms()));
            if left >= MIN_DETOX {
                crate::stop_pomodoro(app);
                crate::start_lock(app, left, SessionKind::Detox);
            }
        }
    }
}

/// "Sync with paired devices", in the Sync tab or the tray menu
pub fn toggle(app: &Rc<App>) {
    let mut config = app.sharing.config.borrow_mut();
    if config.group().is_none() {
        return;
    }
    config.enabled = !config.enabled;
    config.save();
    let enabled = config.enabled;
    drop(config);
    log!("[sync] sync turned {}", if enabled { "on" } else { "off" });
    if let Some(net) = app.sharing.net.borrow().as_ref() {
        net.set_enabled(enabled);
    }
    update_view(app);
}

/// "Pair a device": shows a code to type on the other device
fn invite(app: &Rc<App>) {
    let mut config = app.sharing.config.borrow_mut();
    let group = config.group().unwrap_or_else(|| {
        let group = Group::random();
        config.set_group(Some(&group));
        config.save();
        group
    });
    drop(config);
    if !start_net(app) {
        return;
    }
    let code = sync::random_code();
    if let Some(net) = app.sharing.net.borrow().as_ref() {
        net.set_group(Some(group));
        net.invite(code.clone());
    }
    // Shown as "482 913", easier to read and type
    app.window.set_pairing_code(format!("{} {}", &code[..3], &code[3..]).into());
    set_pairing(app, Pairing::ShowingCode, "");
}

/// The code from the other device has been typed
fn join(app: &Rc<App>, text: &str) {
    let code: String = text.chars().filter(char::is_ascii_digit).collect();
    if code.len() != 6 {
        return set_pairing(app, Pairing::TypingCode, "The code has 6 digits.");
    }
    if !start_net(app) {
        return;
    }
    if let Some(net) = app.sharing.net.borrow().as_ref() {
        net.join(code);
    }
    set_pairing(app, Pairing::Joining, "Looking for the other device…");
}

fn unpair(app: &Rc<App>) {
    let mut config = app.sharing.config.borrow_mut();
    config.set_group(None);
    config.enabled = false;
    config.save();
    drop(config);
    log!("[sync] unpaired");
    if let Some(net) = app.sharing.net.borrow().as_ref() {
        net.set_group(None);
    }
    set_pairing(app, Pairing::None, "This device is no longer paired.");
}

/// Starts the network thread if it isn't running. False if the network can't be used.
fn start_net(app: &Rc<App>) -> bool {
    if app.sharing.net.borrow().is_some() {
        return true;
    }
    let config = app.sharing.config.borrow();
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows PC".into());
    let started = sync::Sync::start(config.device, name, config.group(), config.enabled, |event| {
        crate::dispatch(Action::Sync(event))
    });
    drop(config);
    match started {
        Ok(net) => {
            let (version, shared) = app.sharing.current.get();
            net.publish(version, shared);
            *app.sharing.net.borrow_mut() = Some(net);
            true
        }
        Err(e) => {
            log!("[sync] can't use the network: {e}");
            set_pairing(app, Pairing::None, &format!("Can't use the network: {e}"));
            false
        }
    }
}

fn set_pairing(app: &App, pairing: Pairing, status: &str) {
    app.sharing.pairing.set(pairing);
    app.window.set_pairing_mode(pairing as i32);
    app.window.set_pairing_status(status.into());
    if pairing != Pairing::None {
        app.window.set_tab(2);
    }
}

/// Refreshes the Sync tab and the tray menu
pub fn update_view(app: &App) {
    let config = app.sharing.config.borrow();
    let seen = app.sharing.seen.borrow();
    let devices: Vec<SyncDevice> = config
        .peers
        .iter()
        .map(|peer| SyncDevice {
            name: SharedString::from(peer.name.as_str()),
            online: seen.get(&peer.id).is_some_and(|t| t.elapsed() < ONLINE_FOR),
        })
        .collect();
    let has_group = config.group().is_some();
    let paired = has_group && !devices.is_empty();
    let window = &app.window;
    window.set_sync_paired(paired);
    window.set_sync_has_group(has_group);
    window.set_sync_enabled(config.enabled);
    window.set_sync_devices(ModelRc::new(VecModel::from(devices)));
    if let Some(tray) = &app.tray {
        tray.set_sync(paired, config.enabled);
    }
}
