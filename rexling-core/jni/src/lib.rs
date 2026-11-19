mod activity;
mod progress;
mod engine;
mod updates;
mod uploads;
mod forwards;
use engine::Session;
use jni::{
    JNIEnv,
    objects::{JClass, JString},
    sys::{jlong, jstring, jbyteArray},
};
use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

static SESSIONS: OnceLock<Mutex<HashMap<u64, Session>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn sessions() -> &'static Mutex<HashMap<u64, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn string(env: &mut JNIEnv, value: String) -> jstring {
    env.new_string(value)
        .map(JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_create(
    mut env: JNIEnv,
    _class: JClass,
) -> jlong {
    let result = std::panic::catch_unwind(|| -> anyhow::Result<u64> {
        let session = Session::spawn()?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        sessions()
            .lock()
            .map_err(|_| anyhow::anyhow!("session registry unavailable"))?
            .insert(id, session);
        Ok(id)
    });
    match result {
        Ok(Ok(id)) => id as jlong,
        _ => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Could not start terminal worker",
            );
            0
        }
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_command(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    request: JString,
) -> jstring {
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<()> {
            let request: String = env.get_string(&request)?.into();
            let value = serde_json::from_str(&request)?;
            let sessions = sessions()
                .lock()
                .map_err(|_| anyhow::anyhow!("session registry unavailable"))?;
            sessions
                .get(&(handle as u64))
                .ok_or_else(|| anyhow::anyhow!("terminal session is closed"))?
                .command(value)
        }));
    let message = match result {
        Ok(Ok(())) => String::new(),
        Ok(Err(e)) => e.to_string(),
        Err(_) => "Native command failed".into(),
    };
    string(&mut env, message)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_poll(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    after: jlong,
) -> jstring {
    let result = std::panic::catch_unwind(|| -> anyhow::Result<String> {
        let sessions = sessions()
            .lock()
            .map_err(|_| anyhow::anyhow!("session registry unavailable"))?;
        sessions
            .get(&(handle as u64))
            .ok_or_else(|| anyhow::anyhow!("terminal session is closed"))?
            .poll(after.max(0) as u64)
    });
    string(
        &mut env,
        match result {
            Ok(Ok(value)) => value,
            _ => String::new(),
        },
    )
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    let _ = std::panic::catch_unwind(|| {
        if let Ok(mut sessions) = sessions().lock() {
            sessions.remove(&(handle as u64));
        }
        if let Some(Ok(mut transfers)) = TRANSFERS.get().map(Mutex::lock) {
            transfers.remove(&(handle as u64));
        }
    });
}

/// Wait without holding the handle registry lock. Destroy wakes pending readers.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_awaitUpdate(
    _env: JNIEnv, _class: JClass, handle: jlong, after: jlong, timeout_ms: jlong,
) -> jlong {
    std::panic::catch_unwind(|| {
        let notifier=sessions().lock().ok().and_then(|s|s.get(&(handle as u64)).map(|s|s.updates.clone()));
        notifier.map_or(-1,|n|n.wait(after.max(0) as u64,timeout_ms.clamp(1,1000) as u64))
    }).unwrap_or(-1)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_readUpdate(
    mut env: JNIEnv, _class: JClass, handle: jlong, after: jlong,
) -> jbyteArray {
    let result=std::panic::catch_unwind(|| -> anyhow::Result<Vec<u8>> {
        let shared=sessions().lock().map_err(|_|anyhow::anyhow!("registry unavailable"))?
            .get(&(handle as u64)).map(|s|s.shared.clone()).ok_or_else(||anyhow::anyhow!("closed"))?;
        let mut state=shared.lock().map_err(|_|anyhow::anyhow!("state unavailable"))?;
        Ok(updates::encode(&mut state,after.max(0) as u64))
    });
    match result {Ok(Ok(bytes))=>env.byte_array_from_slice(&bytes).map(|a|a.into_raw()).unwrap_or(std::ptr::null_mut()),
        _=>{let _=env.throw_new("java/lang/IllegalStateException","Native update failed");std::ptr::null_mut()}}
}

/// Packets waiting to be copied out, and the buffers they are built in, by handle.
/// `true` marks a packet the caller's buffer was too small for; the next call returns it.
static TRANSFERS: OnceLock<Mutex<HashMap<u64, (Vec<u8>, bool)>>> = OnceLock::new();
/// Copy the next update into a direct buffer the caller owns and reuses, so a video frame
/// allocates nothing on the Java heap. Returns the packet's length, 0 when there is none, or
/// the negated length when the buffer is too small.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_readUpdateInto(
    mut env: JNIEnv, _class: JClass, handle: jlong, after: jlong, buffer: jni::objects::JByteBuffer,
) -> jni::sys::jint {
    let target=env.get_direct_buffer_address(&buffer).ok().zip(env.get_direct_buffer_capacity(&buffer).ok());
    let result=std::panic::catch_unwind(|| -> anyhow::Result<i32> {
        let (address,capacity)=target.ok_or_else(||anyhow::anyhow!("not a direct buffer"))?;
        let shared=sessions().lock().map_err(|_|anyhow::anyhow!("registry unavailable"))?
            .get(&(handle as u64)).map(|s|s.shared.clone()).ok_or_else(||anyhow::anyhow!("closed"))?;
        let mut transfers=TRANSFERS.get_or_init(||Mutex::new(HashMap::new())).lock().map_err(|_|anyhow::anyhow!("transfer unavailable"))?;
        let (packet,waiting)=transfers.entry(handle as u64).or_default();
        if !*waiting {
            let mut state=shared.lock().map_err(|_|anyhow::anyhow!("state unavailable"))?;
            *packet=updates::encode_reusing(&mut state,after.max(0) as u64,std::mem::take(packet));
        }
        let length=i32::try_from(packet.len())?;
        *waiting=packet.len()>capacity;
        if *waiting {return Ok(-length);}
        // The caller owns `capacity` bytes at `address` for the duration of this call.
        unsafe {std::ptr::copy_nonoverlapping(packet.as_ptr(),address,packet.len());}
        Ok(length)
    });
    match result {Ok(Ok(length))=>length,
        _=>{let _=env.throw_new("java/lang/IllegalStateException","Native update failed");0}}
}

/// Whether every RGBA pixel in a stretch of a direct buffer is opaque. Opaque pictures can be
/// handed to a bitmap as they are; scanning a video frame from Kotlin costs more than drawing it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_opaquePixels(
    env: JNIEnv, _class: JClass, buffer: jni::objects::JByteBuffer, offset: jni::sys::jint, length: jni::sys::jint,
) -> jni::sys::jboolean {
    let target=env.get_direct_buffer_address(&buffer).ok().zip(env.get_direct_buffer_capacity(&buffer).ok());
    let opaque=target.is_some_and(|(address,capacity)| {
        let (offset,length)=(offset.max(0) as usize,length.max(0) as usize);
        if offset.checked_add(length).is_none_or(|end|end>capacity) {return false;}
        // The caller owns the buffer for the duration of this call, and the range is inside it.
        let pixels=unsafe {std::slice::from_raw_parts(address.add(offset),length)};
        pixels.as_chunks::<4>().0.iter().all(|pixel|pixel[3]==255)
    });
    u8::from(opaque)
}

mod input_queue;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_uploadCreate(mut env:JNIEnv,_class:JClass,request:JString)->jlong {
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||->anyhow::Result<u64>{
        let request:String=env.get_string(&request)?.into();uploads::create(serde_json::from_str(&request)?)
    }));
    match result {Ok(Ok(id))=>id as jlong,_=>{let _=env.throw_new("java/lang/IllegalStateException","Could not start image upload");0}}
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_uploadPoll(mut env:JNIEnv,_class:JClass,handle:jlong)->jstring {
    string(&mut env,uploads::poll(handle as u64).to_string())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_uploadDestroy(_env:JNIEnv,_class:JClass,handle:jlong) {uploads::destroy(handle as u64);}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_forwardCreate(mut env: JNIEnv, _class: JClass, request: JString) -> jlong {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<u64> {
        let request: String = env.get_string(&request)?.into();
        forwards::create(serde_json::from_str(&request)?)
    }));
    match result {
        Ok(Ok(id)) => id as jlong,
        error => {
            let message = match error {Ok(Err(e))=>e.to_string(),_=>"Could not start forward".into()};
            let _=env.throw_new("java/lang/IllegalStateException",message);
            0
        }
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_forwardPoll(mut env: JNIEnv, _class: JClass, id: jlong) -> jstring {
    let value = std::panic::catch_unwind(||forwards::poll(id as u64)).unwrap_or_else(|_|serde_json::json!({"phase":"failed"}));
    string(&mut env,value.to_string())
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_forwardDestroy(_env: JNIEnv, _class: JClass, id: jlong) {
    let _=std::panic::catch_unwind(||forwards::destroy(id as u64));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_shulhi_rex_NativeBridge_networkCommand(mut env: JNIEnv, _class: JClass, request: JString) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<String> {
        let request: String = env.get_string(&request)?.into();
        #[cfg(target_os = "android")]
        return rex_core::network::command(&request);
        #[cfg(not(target_os = "android"))]
        { let _ = request; anyhow::bail!("Android networking bridge is unavailable on this platform") }
    }));
    let response = match result {Ok(Ok(v))=>v,Ok(Err(e))=>serde_json::json!({"error":e.to_string()}).to_string(),Err(_)=>"{\"error\":\"Network command failed\"}".into()};
    string(&mut env,response)
}
