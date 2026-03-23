#![allow(clippy::missing_safety_doc)]

const MAX_SESSION_DATA_SIZE: usize = 1024 * 1024;
const MAX_MODULE_NAME_LEN: usize = 256;
const MAX_TTL_SECS: u64 = 86400;

type SessionStore = ::std::sync::Arc<
    crate::storage::redis_hashmap::RedisHashMap<
        crate::storage::module_session_key::ModuleSessionKey,
        Vec<u8>,
    >,
>;

type MetricsStore = ::std::sync::Arc<
    crate::storage::redis_hashmap::RedisHashMap<
        crate::storage::module_session_key::ModuleSessionKey,
        Vec<u8>,
    >,
>;

static MODULE_SESSIONS: ::std::sync::OnceLock<SessionStore> = ::std::sync::OnceLock::new();
static MODULE_METRICS: ::std::sync::OnceLock<MetricsStore> = ::std::sync::OnceLock::new();

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Thread-local allocation tracking
thread_local! {
    static ALLOCATED_SESSIONS: scc::HashMap<(u64, usize), usize> = scc::HashMap::new();
}

fn get_thread_id() -> u64 {
    let mut hasher = DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}

pub fn set_module_sessions(sessions: SessionStore) {
    let _ = MODULE_SESSIONS.set(sessions);
}

pub fn set_module_metrics(metrics: MetricsStore) {
    let _ = MODULE_METRICS.set(metrics);
}

pub unsafe extern "C" fn host_fn_log(
    level: u8,
    target: *const u8,
    target_len: usize,
    message: *const u8,
    message_len: usize,
) -> u8 {
    unsafe {
        let target_str = if !target.is_null() && target_len > 0 {
            ::std::str::from_utf8(::std::slice::from_raw_parts(target, target_len))
                .unwrap_or("unknown")
        } else {
            "unknown"
        };

        let message_str = if !message.is_null() && message_len > 0 {
            ::std::str::from_utf8(::std::slice::from_raw_parts(message, message_len))
                .unwrap_or_default()
        } else {
            ""
        };

        let formatted = format!("[module: {}] {}", target_str, message_str);

        match level {
            0 => tracing::error!("{}", formatted),
            1 => tracing::warn!("{}", formatted),
            2 => tracing::info!("{}", formatted),
            3 => tracing::debug!("{}", formatted),
            _ => tracing::trace!("{}", formatted),
        }
    }
    0
}

pub unsafe extern "C" fn host_session_get(
    session_id: *const u8,
    module_name: *const u8,
    module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> bool {
    if session_id.is_null() || module_name.is_null() || out_ptr.is_null() || out_len.is_null() {
        return false;
    }

    if module_name_len > MAX_MODULE_NAME_LEN {
        return false;
    }

    let store: &SessionStore = match MODULE_SESSIONS.get() {
        Some(s) => s,
        None => return false,
    };

    let session_id_slice = unsafe { ::std::slice::from_raw_parts(session_id, 16) };
    let session_id_array: [u8; 16] = match session_id_slice.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let session_uuid = uuid::Uuid::from_bytes(session_id_array);

    let module_slice = unsafe { ::std::slice::from_raw_parts(module_name, module_name_len) };
    let module_key = match ::std::str::from_utf8(module_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key_slice = unsafe { ::std::slice::from_raw_parts(data_key, data_key_len) };
    let data_key_str = match ::std::str::from_utf8(key_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key = crate::storage::module_session_key::ModuleSessionKey::new(
        &format!("{}:{}", module_key, data_key_str),
        session_uuid,
    );

    if let Some(data) = store.get_hot_or_cold_sync(&key) {
        let data_len = data.len();
        let boxed = Box::into_raw(data.into_boxed_slice());
        let data_ptr = boxed as *mut u8;

        // Track allocation for later cleanup
        let thread_id = get_thread_id();
        let ptr_addr = data_ptr as usize;
        ALLOCATED_SESSIONS.with(|map| {
            let _ = map.insert_sync((thread_id, ptr_addr), data_len);
        });

        unsafe {
            *out_ptr = data_ptr;
            *out_len = data_len;
        }
        return true;
    }

    false
}

pub unsafe extern "C" fn host_session_set(
    session_id: *const u8,
    module_name: *const u8,
    module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    data_ptr: *const u8,
    data_len: usize,
) -> bool {
    if session_id.is_null() || module_name.is_null() || data_ptr.is_null() {
        return false;
    }

    if module_name_len > MAX_MODULE_NAME_LEN {
        return false;
    }

    if data_len > MAX_SESSION_DATA_SIZE {
        return false;
    }

    let store: &SessionStore = match MODULE_SESSIONS.get() {
        Some(s) => s,
        None => return false,
    };

    let session_id_slice = unsafe { ::std::slice::from_raw_parts(session_id, 16) };
    let session_id_array: [u8; 16] = match session_id_slice.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let session_uuid = uuid::Uuid::from_bytes(session_id_array);

    let module_slice = unsafe { ::std::slice::from_raw_parts(module_name, module_name_len) };
    let module_key = match ::std::str::from_utf8(module_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key_slice = unsafe { ::std::slice::from_raw_parts(data_key, data_key_len) };
    let data_key_str = match ::std::str::from_utf8(key_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key = crate::storage::module_session_key::ModuleSessionKey::new(
        &format!("{}:{}", module_key, data_key_str),
        session_uuid,
    );
    let data_slice = unsafe { ::std::slice::from_raw_parts(data_ptr, data_len) };
    let data_vec = data_slice.to_vec();

    store.set_sync(key.clone(), data_vec.clone());
    let _ = store.set_redis_sync(key, data_vec);
    true
}

pub unsafe extern "C" fn host_session_set_with_ttl(
    session_id: *const u8,
    module_name: *const u8,
    module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    data_ptr: *const u8,
    data_len: usize,
    ttl_secs: u64,
) -> bool {
    if session_id.is_null() || module_name.is_null() || data_ptr.is_null() {
        return false;
    }

    if module_name_len > MAX_MODULE_NAME_LEN {
        return false;
    }

    if data_len > MAX_SESSION_DATA_SIZE {
        return false;
    }

    let ttl = ttl_secs.min(MAX_TTL_SECS);

    let store: &SessionStore = match MODULE_SESSIONS.get() {
        Some(s) => s,
        None => return false,
    };

    let session_id_slice = unsafe { ::std::slice::from_raw_parts(session_id, 16) };
    let session_id_array: [u8; 16] = match session_id_slice.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let session_uuid = uuid::Uuid::from_bytes(session_id_array);

    let module_slice = unsafe { ::std::slice::from_raw_parts(module_name, module_name_len) };
    let module_key = match ::std::str::from_utf8(module_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key_slice = unsafe { ::std::slice::from_raw_parts(data_key, data_key_len) };
    let data_key_str = match ::std::str::from_utf8(key_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key = crate::storage::module_session_key::ModuleSessionKey::new(
        &format!("{}:{}", module_key, data_key_str),
        session_uuid,
    );
    let data_slice = unsafe { ::std::slice::from_raw_parts(data_ptr, data_len) };
    let data_vec = data_slice.to_vec();

    store.set_with_ttl_sync(key.clone(), data_vec.clone(), ttl);
    let _ = store.set_redis_sync_with_ttl(key, data_vec, ttl);
    true
}

pub unsafe extern "C" fn host_session_free(
    _module_name: *const u8,
    _module_name_len: usize,
    ptr: *const u8,
    len: usize,
) {
    if ptr.is_null() || len == 0 {
        return;
    }

    // Find and remove the allocation record
    let thread_id = get_thread_id();
    let ptr_addr = ptr as usize;

    ALLOCATED_SESSIONS.with(|map| {
        let _ = map.remove_sync(&(thread_id, ptr_addr));
    });

    unsafe {
        let slice = ::std::slice::from_raw_parts_mut(ptr as *mut u8, len);
        let _boxed = Box::from_raw(slice);
    }
}

pub unsafe extern "C" fn host_metrics_good_boy(
    metrics_key_ptr: *const u8,
    metrics_key_len: usize,
) -> bool {
    if metrics_key_ptr.is_null() || metrics_key_len != 16 {
        return false;
    }

    let store: &MetricsStore = match MODULE_METRICS.get() {
        Some(s) => s,
        None => return false,
    };

    let uuid_bytes = unsafe { ::std::slice::from_raw_parts(metrics_key_ptr, 16) };
    let uuid_array: [u8; 16] = match uuid_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let session_uuid = uuid::Uuid::from_bytes(uuid_array);
    let key = crate::storage::module_session_key::ModuleSessionKey::user_session(session_uuid);

    if let Some(score_bytes) = store.get_hot_or_cold_sync(&key)
        && let Ok(score) = rmp_serde::from_slice::<crate::metrics::AtomicU64Wrapper>(&score_bytes)
    {
        let new_value = crate::metrics::AtomicU64Wrapper::new(score.load().saturating_sub(50));
        if let Ok(encoded) = rmp_serde::to_vec(&new_value) {
            let _ = store.set_sync(key.clone(), encoded.clone());
            let _ = store.set_redis_sync(key, encoded);
            return true;
        }
    }

    false
}

pub unsafe extern "C" fn host_metrics_get_score(
    metrics_key_ptr: *const u8,
    metrics_key_len: usize,
) -> u64 {
    if metrics_key_ptr.is_null() || metrics_key_len != 16 {
        return 0;
    }

    let store: &MetricsStore = match MODULE_METRICS.get() {
        Some(s) => s,
        None => return 0,
    };

    let uuid_bytes = unsafe { ::std::slice::from_raw_parts(metrics_key_ptr, 16) };
    let uuid_array: [u8; 16] = match uuid_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return 0,
    };
    let session_uuid = uuid::Uuid::from_bytes(uuid_array);
    let key = crate::storage::module_session_key::ModuleSessionKey::user_session(session_uuid);

    store
        .get_hot_or_cold_sync(&key)
        .and_then(|score_bytes| {
            rmp_serde::from_slice::<crate::metrics::AtomicU64Wrapper>(&score_bytes)
                .ok()
                .map(|s| s.load())
        })
        .unwrap_or(0)
}
