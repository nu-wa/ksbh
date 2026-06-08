use ksbh_modules_abi::{
    functions::KSBHHostSignalKind,
    prelude::{KSBHBytes, KSBHHostFnReturn, KSBHString},
    types::KSBHHostCtxHandle,
};

use super::active_call::{Keyspace, resolve_active_call};

const MAX_SESSION_DATA_SIZE: usize = 1024 * 1024;

pub(crate) unsafe extern "C" fn host_fn_log(
    ctx_handle: KSBHHostCtxHandle,
    level: ksbh_modules_abi::types::LogLevel,
    message: KSBHBytes,
) -> KSBHHostFnReturn {
    let active_call = match resolve_active_call(ctx_handle) {
        Ok(call) => call,
        Err(ret) => return ret,
    };

    let message = ::std::str::from_utf8(message.as_slice()).unwrap_or_default();
    let formatted = format!("[module: {}] {}", active_call.module_name, message);

    match level {
        ksbh_modules_abi::types::LogLevel::Error => tracing::error!("{}", formatted),
        ksbh_modules_abi::types::LogLevel::Warn => tracing::warn!("{}", formatted),
        ksbh_modules_abi::types::LogLevel::Info => tracing::info!("{}", formatted),
        ksbh_modules_abi::types::LogLevel::Debug => tracing::debug!("{}", formatted),
        ksbh_modules_abi::types::LogLevel::Trace => tracing::trace!("{}", formatted),
    }

    KSBHHostFnReturn::Success
}

pub(crate) unsafe extern "C" fn host_fn_free_bytes(ctx_handle: KSBHHostCtxHandle, data: KSBHBytes) {
    let active_call = match resolve_active_call(ctx_handle) {
        Ok(call) => call,
        Err(_) => return,
    };

    if data.ptr.is_null() || data.len == 0 {
        return;
    }

    active_call.free_session_buffer(data.ptr);
}

pub(crate) unsafe extern "C" fn host_fn_reputation_good_boy(
    ctx_handle: KSBHHostCtxHandle,
) -> KSBHHostFnReturn {
    let active_call = match resolve_active_call(ctx_handle) {
        Ok(call) => call,
        Err(ret) => return ret,
    };

    active_call
        .session_store
        .reputation_good_boy(active_call.reputation_key, active_call.client_ip);

    KSBHHostFnReturn::Success
}

pub(crate) unsafe extern "C" fn host_fn_reputation_get_score(
    ctx_handle: KSBHHostCtxHandle,
    out: *mut u64,
) -> KSBHHostFnReturn {
    if out.is_null() {
        return KSBHHostFnReturn::BadArgument;
    }

    let active_call = match resolve_active_call(ctx_handle) {
        Ok(call) => call,
        Err(ret) => return ret,
    };

    let score = active_call
        .session_store
        .reputation_get_score(active_call.reputation_key, active_call.client_ip);

    unsafe {
        *out = score;
    }

    KSBHHostFnReturn::Success
}

/// Returns a host-wide runtime signal (in-flight requests, recent error rate, etc).
///
/// `RUNTIME_SIGNALS` is a process-global counter, not a per-call field: every
/// `request_started` / `observe_completion` writes to the same static. The
/// `ctx_handle` is checked only as a liveness guard — the calling module must
/// still have an active call registered — and the result is not derived from
/// the per-call context.
pub(crate) unsafe extern "C" fn host_fn_signal_get(
    ctx_handle: KSBHHostCtxHandle,
    kind: KSBHHostSignalKind,
    out: *mut u64,
) -> KSBHHostFnReturn {
    if out.is_null() {
        return KSBHHostFnReturn::BadArgument;
    }

    if resolve_active_call(ctx_handle).is_err() {
        return KSBHHostFnReturn::NotFound;
    }

    let signal = crate::metrics::runtime_signals::RUNTIME_SIGNALS.get(kind);

    unsafe {
        *out = signal;
    }

    KSBHHostFnReturn::Success
}

pub(crate) fn host_fn_session_get(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    keyspace: Keyspace,
    out_ptr: *mut KSBHBytes,
) -> KSBHHostFnReturn {
    if out_ptr.is_null() {
        return KSBHHostFnReturn::BadArgument;
    }

    let active_call = match resolve_active_call(ctx_handle) {
        Ok(call) => call,
        Err(ret) => return ret,
    };

    let key_bytes = key.as_slice();
    let Ok(data_key) = ::std::str::from_utf8(key_bytes) else {
        return KSBHHostFnReturn::BadArgument;
    };

    let session_key = active_call.session_key(data_key, keyspace);
    let data = tokio::task::block_in_place(|| {
        active_call.session_store.get_hot_or_cold_sync(&session_key)
    });
    let Some(data) = data else {
        return KSBHHostFnReturn::NotFound;
    };

    let data_vec = data;
    let bytes = KSBHBytes {
        ptr: data_vec.as_ptr(),
        len: data_vec.len(),
    };
    active_call.track_session_buffer(bytes.ptr, data_vec);

    unsafe {
        *out_ptr = bytes;
    }

    KSBHHostFnReturn::Success
}

pub(crate) fn host_fn_session_set(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    value: KSBHBytes,
    ttl: u64,
    keyspace: Keyspace,
) -> KSBHHostFnReturn {
    let active_call = match resolve_active_call(ctx_handle) {
        Ok(call) => call,
        Err(ret) => return ret,
    };

    let key_bytes = key.as_slice();
    let value_bytes = value.as_slice();
    if value_bytes.len() > MAX_SESSION_DATA_SIZE {
        return KSBHHostFnReturn::BadArgument;
    }

    let Ok(data_key) = ::std::str::from_utf8(key_bytes) else {
        return KSBHHostFnReturn::BadArgument;
    };

    let session_key = active_call.session_key(data_key, keyspace);
    let data_vec = value_bytes.to_vec();

    // Write to hot cache synchronously (in-memory, fast)
    if ttl == 0 {
        let _ = active_call.session_store.set_sync(session_key.clone(), data_vec.clone());
    } else {
        let _ = active_call.session_store.set_with_ttl_sync(session_key.clone(), data_vec.clone(), ttl);
    }

    // Fire-and-forget Redis write — no blocking, returns immediately
    let store = active_call.session_store.clone();
    let redis_ttl = ttl;
    tokio::spawn(async move {
        if let Some(storage) = store.storage() {
            if let Ok(mut conn) = storage.get_redis_async() {
                let key_bytes = match rmp_serde::to_vec(&session_key) { Ok(k) => k, Err(_) => return };
                let encoded = match rmp_serde::to_vec(&data_vec) { Ok(v) => v, Err(_) => return };
                let ttl_secs = if redis_ttl == 0 { 86400 } else { redis_ttl };
                let _ = redis::cmd("SETEX")
                    .arg(&key_bytes)
                    .arg(ttl_secs)
                    .arg(&encoded)
                    .query_async::<()>(&mut conn)
                    .await;
            }
        }
    });

    KSBHHostFnReturn::Success
}

pub(crate) fn bytes_to_string(value: &[u8]) -> KSBHString {
    KSBHString {
        inner: KSBHBytes::from_slice(value),
    }
}

pub(crate) unsafe extern "C" fn host_fn_session_get_per_module(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    out_ptr: *mut KSBHBytes,
) -> KSBHHostFnReturn {
    host_fn_session_get(ctx_handle, key, Keyspace::PerModule, out_ptr)
}

pub(crate) unsafe extern "C" fn host_fn_session_set_per_module(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    value: KSBHBytes,
    ttl: u64,
) -> KSBHHostFnReturn {
    host_fn_session_set(ctx_handle, key, value, ttl, Keyspace::PerModule)
}

pub(crate) unsafe extern "C" fn host_fn_session_get_shared(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    out_ptr: *mut KSBHBytes,
) -> KSBHHostFnReturn {
    host_fn_session_get(ctx_handle, key, Keyspace::Shared, out_ptr)
}

pub(crate) unsafe extern "C" fn host_fn_session_set_shared(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    value: KSBHBytes,
    ttl: u64,
) -> KSBHHostFnReturn {
    host_fn_session_set(ctx_handle, key, value, ttl, Keyspace::Shared)
}
