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
        crate::proxy::PartialClientInformation,
        crate::metrics::Hits,
    >,
>;

static MODULE_SESSIONS: ::std::sync::OnceLock<SessionStore> = ::std::sync::OnceLock::new();
static MODULE_METRICS: ::std::sync::OnceLock<MetricsStore> = ::std::sync::OnceLock::new();

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

    if let Some(data) = store.get_sync(&key).or_else(|| store.get_redis_sync(&key)) {
        let data_len = data.len();
        let boxed = Box::into_raw(data.into_boxed_slice());
        let data_ptr = boxed as *mut u8;
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
    if store.set_redis_sync(key, data_vec) {
        return true;
    }
    false
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
    if store.set_redis_sync_with_ttl(key, data_vec, ttl) {
        return true;
    }
    false
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
    unsafe {
        let slice = ::std::slice::from_raw_parts_mut(ptr as *mut u8, len);
        let _boxed = Box::from_raw(slice);
    }
}

pub unsafe extern "C" fn host_metrics_increment_good(
    client_ip: *const u8,
    client_ip_len: usize,
    user_agent: *const u8,
    user_agent_len: usize,
) -> bool {
    if client_ip.is_null() {
        return false;
    }

    let store: &MetricsStore = match MODULE_METRICS.get() {
        Some(s) => s,
        None => return false,
    };

    let ip_slice = unsafe { ::std::slice::from_raw_parts(client_ip, client_ip_len) };
    let ip_str = match ::std::str::from_utf8(ip_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let ip: ::std::net::IpAddr = match ip_str.parse() {
        Ok(a) => a,
        Err(_) => return false,
    };

    let ua = if !user_agent.is_null() && user_agent_len > 0 {
        let ua_slice = unsafe { ::std::slice::from_raw_parts(user_agent, user_agent_len) };
        match ::std::str::from_utf8(ua_slice) {
            Ok(s) => Some(ksbh_types::KsbhStr::new(s)),
            Err(_) => None,
        }
    } else {
        None
    };

    let client_info = crate::proxy::PartialClientInformation { ip, user_agent: ua };

    let current = store
        .get_sync(&client_info)
        .or_else(|| store.get_redis_sync(&client_info));

    let new_good = match current {
        Some(hits) => hits.good + hits.bad + 5,
        None => 5,
    };

    let new_hits = crate::metrics::Hits {
        good: new_good,
        bad: 0,
        logged_at: chrono::Utc::now().naive_utc(),
    };

    store.set_sync(client_info.clone(), new_hits.clone());
    store.set_redis_sync(client_info, new_hits);

    true
}

pub unsafe extern "C" fn host_metrics_get_hits(
    client_ip: *const u8,
    client_ip_len: usize,
    user_agent: *const u8,
    user_agent_len: usize,
    out_good: *mut u32,
    out_bad: *mut u32,
) -> bool {
    if client_ip.is_null() || out_good.is_null() || out_bad.is_null() {
        return false;
    }

    let store: &MetricsStore = match MODULE_METRICS.get() {
        Some(s) => s,
        None => return false,
    };

    let ip_slice = unsafe { ::std::slice::from_raw_parts(client_ip, client_ip_len) };
    let ip_str = match ::std::str::from_utf8(ip_slice) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let ip: ::std::net::IpAddr = match ip_str.parse() {
        Ok(a) => a,
        Err(_) => return false,
    };

    let ua = if !user_agent.is_null() && user_agent_len > 0 {
        let ua_slice = unsafe { ::std::slice::from_raw_parts(user_agent, user_agent_len) };
        match ::std::str::from_utf8(ua_slice) {
            Ok(s) => Some(ksbh_types::KsbhStr::new(s)),
            Err(_) => None,
        }
    } else {
        None
    };

    let client_info = crate::proxy::PartialClientInformation { ip, user_agent: ua };

    let hits = store
        .get_sync(&client_info)
        .or_else(|| store.get_redis_sync(&client_info));

    if let Some(hits) = hits {
        unsafe {
            *out_good = hits.good as u32;
            *out_bad = hits.bad as u32;
        }
        return true;
    }

    unsafe {
        *out_good = 0;
        *out_bad = 0;
    };
    true
}
