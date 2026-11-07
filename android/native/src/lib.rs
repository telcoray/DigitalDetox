use std::sync::{Mutex, OnceLock, mpsc};
use jni::{JNIEnv, objects::{JClass,JString}, sys::jstring};
use serde_json::{Value, json};
macro_rules! log { ($($arg:tt)*) => {{ let _ = format_args!($($arg)*); }}; }
mod sync { include!(concat!(env!("OUT_DIR"), "/sync.rs")); }
struct Engine { net: sync::Sync, events: mpsc::Receiver<sync::Event> }
static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static ADDRESSES: OnceLock<Mutex<Vec<std::net::Ipv4Addr>>> = OnceLock::new();
fn broadcasts() -> Vec<std::net::Ipv4Addr> { ADDRESSES.get_or_init(|| Mutex::new(vec![])).lock().unwrap().clone() }
fn number(v: &Value) -> Result<u64, String> { v.as_str().ok_or("missing number")?.parse().map_err(|_| "bad number".into()) }
fn group(v: &Value) -> Result<sync::Group,String> {
    let bytes: Vec<u8> = serde_json::from_value(v["key"].clone()).map_err(|e|e.to_string())?;
    Ok(sync::Group {id:number(&v["id"])?,key:bytes.try_into().map_err(|_|"bad key")?})
}
fn rpc(v: Value) -> Result<Value,String> {
    let mut slot=ENGINE.lock().map_err(|_|"engine unavailable")?;
    if v["op"] == "close" { *slot=None; return Ok(json!({})); }
    if v["op"] == "init" {
        if slot.is_some() { return Ok(json!({})); }
        let addresses=v["addresses"].as_array().ok_or("missing addresses")?.iter().filter_map(|a|a.as_str()?.parse().ok()).collect();
        *ADDRESSES.get_or_init(||Mutex::new(vec![])).lock().unwrap()=addresses;
        let (tx,rx)=mpsc::channel();
        let g=if v["group"].is_null(){None}else{Some(group(&v["group"])?) };
        let net=sync::Sync::start(number(&v["device"])?,v["name"].as_str().unwrap_or("Android").into(),g,v["enabled"].as_bool().unwrap_or(false),move|e|{let _=tx.send(e);}).map_err(|e|e.to_string())?;
        net.publish(sync::Version::default(),sync::Shared::Idle);
        *slot=Some(Engine{net,events:rx});
        return Ok(json!({}));
    }
    let e=slot.as_ref().ok_or("Sync is not running")?;
    match v["op"].as_str().unwrap_or("") {
        "publish" => e.net.publish(sync::Version{time:v["time"].as_u64().ok_or("bad time")?,device:number(&v["device"])?},serde_json::from_value(v["shared"].clone()).map_err(|e|e.to_string())?),
        "enable"=>e.net.set_enabled(v["enabled"].as_bool().unwrap_or(false)),
        "group"=>e.net.set_group(if v["group"].is_null(){None}else{Some(group(&v["group"])?) }),
        "join"=> { let code=v["code"].as_str().ok_or("missing code")?; if code.len()!=6 || !code.bytes().all(|x|x.is_ascii_digit()){return Err("Enter six digits".into());} e.net.join(code.into()); },
        "invite"=>{let code=sync::random_code();e.net.invite(code.clone());return Ok(json!({"code":code}));},
        "cancel"=>e.net.cancel_pairing(),
        "poll"=>{
            let events:Vec<_>=e.events.try_iter().map(|event|match event {
                sync::Event::Remote{device,name,version,shared}=>json!({"type":"remote","device":device.to_string(),"name":name,"time":version.time,"author":version.device.to_string(),"shared":shared}),
                sync::Event::Paired{device,name,group}=>json!({"type":"paired","device":device.to_string(),"name":name,"group":group.map(|g|json!({"id":g.id.to_string(),"key":g.key.to_vec()}))}),
                sync::Event::PairingFailed(reason)=>json!({"type":"error","message":reason})
            }).collect();return Ok(json!({"events":events}));
        },
        _=>return Err("unknown operation".into())
    }
    Ok(json!({}))
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_telcoray_digitaldetox_NativeSync_call(mut env:JNIEnv,_class:JClass,input:JString)->jstring {
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let input:String=env.get_string(&input).map_err(|e|e.to_string())?.into();
        rpc(serde_json::from_str(&input).map_err(|e|e.to_string())?)
    })).unwrap_or_else(|_|Err("Native Sync failure".into())).unwrap_or_else(|e|json!({"error":e}));
    env.new_string(result.to_string()).map(|s|s.into_raw()).unwrap_or(std::ptr::null_mut())
}
