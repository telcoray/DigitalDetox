//! The Sync worker of the Windows app, built for Android.
//!
//! build.rs copies `src/sync/mod.rs` and `protocol.rs` from the Windows app, replacing only
//! the parts that depend on Windows (the saved configuration and the list of broadcast
//! addresses), so both apps speak exactly the same protocol.
//!
//! Java talks to it through a single native function, `NativeSync.call`, which takes a JSON
//! request (`{"op": "init", ...}`) and returns a JSON answer (`{"error": "..."}` on failure).
//! Device and group ids are 64-bit unsigned numbers, which Java's `long` can't hold, so they
//! travel as strings.

use std::net::Ipv4Addr;
use std::sync::{Mutex, mpsc};

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use serde_json::{Value, json};

// The worker logs with `log!`; Android has no log file yet, so the messages are dropped
macro_rules! log {
    ($($arg:tt)*) => {{
        let _ = format_args!($($arg)*);
    }};
}

// Some of it is only used on Windows (e.g. creating a group, which Java does here, or the
// Pomodoro plan, which Java reads from the JSON)
#[allow(dead_code, unused_imports)]
mod sync {
    include!(concat!(env!("OUT_DIR"), "/sync.rs"));
}

/// The running worker, and the events it has sent that Java hasn't collected yet
struct Engine {
    net: sync::Sync,
    events: mpsc::Receiver<sync::Event>,
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
/// Found by Java (Android has no GetAdaptersAddresses), read by the worker through `net`
static ADDRESSES: Mutex<Vec<Ipv4Addr>> = Mutex::new(Vec::new());

/// The broadcast addresses of the networks the phone is on
fn broadcasts() -> Vec<Ipv4Addr> {
    ADDRESSES.lock().map(|a| a.clone()).unwrap_or_default()
}

/// A 64-bit id sent as a string
fn number(value: &Value) -> Result<u64, String> {
    value.as_str().ok_or("missing number")?.parse().map_err(|_| "bad number".into())
}

/// `{"id": "...", "key": [32 bytes]}`, or null for no group
fn group(value: &Value) -> Result<Option<sync::Group>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let key: Vec<u8> = serde_json::from_value(value["key"].clone()).map_err(|e| e.to_string())?;
    let key = key.try_into().map_err(|_| "bad key")?;
    Ok(Some(sync::Group { id: number(&value["id"])?, key }))
}

fn event_to_json(event: sync::Event) -> Value {
    match event {
        sync::Event::Remote { device, name, version, shared } => json!({
            "type": "remote",
            "device": device.to_string(),
            "name": name,
            "time": version.time,
            "author": version.device.to_string(),
            "shared": shared,
        }),
        sync::Event::Paired { device, name, group } => json!({
            "type": "paired",
            "device": device.to_string(),
            "name": name,
            "group": group.map(|g| json!({ "id": g.id.to_string(), "key": g.key.to_vec() })),
        }),
        sync::Event::PairingFailed(reason) => json!({ "type": "error", "message": reason }),
    }
}

fn handle(request: Value) -> Result<Value, String> {
    let mut slot = ENGINE.lock().map_err(|_| "engine unavailable")?;
    match request["op"].as_str().unwrap_or("") {
        "init" => {
            // Already running: the service was started again
            if slot.is_some() {
                return Ok(json!({}));
            }
            let addresses = request["addresses"].as_array().ok_or("missing addresses")?;
            *ADDRESSES.lock().map_err(|_| "addresses unavailable")? =
                addresses.iter().filter_map(|a| a.as_str()?.parse().ok()).collect();
            let (sender, events) = mpsc::channel();
            let net = sync::Sync::start(
                number(&request["device"])?,
                request["name"].as_str().unwrap_or("Android").into(),
                group(&request["group"])?,
                request["enabled"].as_bool().unwrap_or(false),
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .map_err(|e| e.to_string())?;
            net.publish(sync::Version::default(), sync::Shared::Idle);
            *slot = Some(Engine { net, events });
            return Ok(json!({}));
        }
        "close" => {
            *slot = None;
            return Ok(json!({}));
        }
        _ => {}
    }

    let engine = slot.as_ref().ok_or("Sync is not running")?;
    match request["op"].as_str().unwrap_or("") {
        "publish" => {
            let version = sync::Version {
                time: request["time"].as_u64().ok_or("bad time")?,
                device: number(&request["device"])?,
            };
            let shared = serde_json::from_value(request["shared"].clone()).map_err(|e| e.to_string())?;
            engine.net.publish(version, shared);
        }
        "enable" => engine.net.set_enabled(request["enabled"].as_bool().unwrap_or(false)),
        "group" => engine.net.set_group(group(&request["group"])?),
        "join" => {
            let code = request["code"].as_str().ok_or("missing code")?;
            if code.len() != 6 || !code.bytes().all(|c| c.is_ascii_digit()) {
                return Err("Enter six digits".into());
            }
            engine.net.join(code.into());
        }
        "invite" => {
            let code = sync::random_code();
            engine.net.invite(code.clone());
            return Ok(json!({ "code": code }));
        }
        "cancel" => engine.net.cancel_pairing(),
        "poll" => {
            let events: Vec<Value> = engine.events.try_iter().map(event_to_json).collect();
            return Ok(json!({ "events": events }));
        }
        _ => return Err("unknown operation".into()),
    }
    Ok(json!({}))
}

/// `NativeSync.call(String)`. A panic must not cross into Java, so it becomes an error answer.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_telcoray_digitaldetox_NativeSync_call(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let input: String = env.get_string(&input).map_err(|e| e.to_string())?.into();
        handle(serde_json::from_str(&input).map_err(|e| e.to_string())?)
    }))
    .unwrap_or_else(|_| Err("Native Sync failure".into()))
    .unwrap_or_else(|e| json!({ "error": e }));
    env.new_string(result.to_string()).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}
