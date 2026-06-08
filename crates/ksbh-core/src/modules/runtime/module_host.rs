use std::{
    net::IpAddr,
    sync::Arc,
};

use ksbh_modules_abi::{
    convert::build_kv_entries,
    prelude::{
        KSBHBytes, KSBHModuleDecision, KSBHSliceKVStrings, KSBHString,
        ModuleContext, ModuleResponse, RequestInfo,
    },
    types::{KSBHHostCtxHandle, KSBHKVString, SessionID},
};

use super::{ModuleCallInput, ModuleCallOutcome};
use super::active_call::{ACTIVE_CALLS, ActiveCallGuard, ActiveModuleCall};
use super::host_functions::{
    bytes_to_string, host_fn_free_bytes, host_fn_log, host_fn_reputation_get_score,
    host_fn_reputation_good_boy, host_fn_session_get_per_module, host_fn_session_get_shared,
    host_fn_session_set_per_module, host_fn_session_set_shared,
    host_fn_signal_get,
};

/// Errors that can occur when interacting with module host.
#[derive(Debug, thiserror::Error)]
pub enum ModuleHostError {
    /// Requested module is not loaded
    #[error("module not found")]
    ModuleNotFound,
    /// Module returned an error
    #[error("{0}")]
    ModuleError(String),
    /// Internal host error
    #[error("{0}")]
    InternalError(String),
}

/// Hosts loaded modules and dispatches requests to them.
pub struct ModuleHost {
    modules: scc::HashMap<
        crate::modules::ModuleConfigurationType,
        super::module_instance::ModuleInstance,
    >,
    /// Reverse index: file path → module type, used for delete events
    pub module_paths: scc::HashMap<std::path::PathBuf, crate::modules::ModuleConfigurationType>,
    store: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
}

impl ModuleHost {
    pub fn new(
        store: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    ) -> Self {
        Self {
            modules: scc::HashMap::new(),
            module_paths: scc::HashMap::new(),
            store,
        }
    }

    pub fn session_store(
        &self,
    ) -> &::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    > {
        &self.store
    }

    pub fn reputation_observe(
        &self,
        reputation_key: [u8; 32],
        client_ip: Option<IpAddr>,
        delta: u64,
    ) {
        self.store
            .reputation_observe(reputation_key, client_ip, delta);
    }

    pub fn reputation_good_boy(&self, reputation_key: [u8; 32], client_ip: Option<IpAddr>) {
        self.store.reputation_good_boy(reputation_key, client_ip);
    }

    pub fn reputation_get_score(&self, reputation_key: [u8; 32], client_ip: Option<IpAddr>) -> u64 {
        self.store.reputation_get_score(reputation_key, client_ip)
    }
}

impl ModuleHost {
    pub fn load_module<P: AsRef<::std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), super::error::AbiError> {
        let module_instance = super::module_instance::ModuleInstance::load(&path)?;
        let mod_type = module_instance.mod_type.clone();
        let path_buf = path.as_ref().to_path_buf();

        let old = self.modules.upsert_sync(mod_type.clone(), module_instance);

        if let Some(old_instance) = old {
            tracing::info!(
                "Reloaded module type {:?} (old: {}, new: {})",
                mod_type,
                old_instance.file_name,
                path_buf.display(),
            );
        } else {
            tracing::info!("Loaded module type {:?} from {}", mod_type, path_buf.display());
        }

        self.module_paths.upsert_sync(path_buf, mod_type);

        Ok(())
    }

    pub fn unload_module(
        &self,
        mod_type: &crate::modules::ModuleConfigurationType,
    ) {
        if let Some((_, instance)) = self.modules.remove_sync(mod_type) {
            tracing::info!("Unloaded module type {:?} (was: {})", mod_type, instance.file_name);
        }
        let mut entry = self.module_paths.begin_sync();
        while let Some(occupied_entry) = entry {
            if occupied_entry.get() == mod_type {
                self.module_paths.remove_sync(occupied_entry.key());
            }
            entry = occupied_entry.next_sync();
        }
    }

    pub fn module_type_for_path(&self, path: &std::path::Path) -> Option<crate::modules::ModuleConfigurationType> {
        self.module_paths.get_sync(path).map(|entry| entry.get().clone())
    }

    pub fn call_module(
        &self,
        input: ModuleCallInput<'_>,
    ) -> Result<ModuleCallOutcome, ModuleHostError> {
        let module = match self.modules.get_sync(&input.module_type) {
            Some(module) => module,
            None => return Err(ModuleHostError::ModuleNotFound),
        };

        let active_call = Arc::new(ActiveModuleCall::new(
            input.module_name.to_owned(),
            *input.observed.session_id.as_bytes(),
            input.observed.client.reputation_key,
            Some(input.observed.client.ip),
            self.store.clone(),
        ));
        let ctx_handle = ACTIVE_CALLS.insert(active_call);
        let mut active_call_guard = ActiveCallGuard::new(ctx_handle);

        let query_params = build_kv_entries(
            input
                .observed
                .http_request
                .query
                .params
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let request_info = RequestInfo {
            uri: KSBHString::from_str(input.observed.http_request.uri.as_str()),
            host: KSBHString::from_str(input.observed.http_request.host.as_str()),
            method: KSBHString::from_str(input.observed.http_request.method.as_str()),
            path: KSBHString::from_str(input.observed.http_request.query.path.as_str()),
            query_params: KSBHSliceKVStrings {
                ptr: query_params.as_ptr(),
                len: query_params.len(),
            },
            scheme: KSBHString::from_str(input.observed.http_request.scheme.as_str()),
            port: input.observed.http_request.port,
            is_ws_handshake: input.observed.is_websocket_handshake as u8,
        };

        let config_entries = build_kv_entries(
            input
                .config
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let header_entries = build_header_entries(input.headers);
        let body = input.body.map_or(
            KSBHBytes {
                ptr: std::ptr::null(),
                len: 0,
            },
            bytes_from_bytes,
        );
        let cookie_header = input.headers.get(http::header::COOKIE).map_or(
            KSBHString {
                inner: KSBHBytes {
                    ptr: std::ptr::null(),
                    len: 0,
                },
            },
            |value| KSBHString {
                inner: KSBHBytes::from_slice(value.as_bytes()),
            },
        );

        let mut ctx = ModuleContext {
            host_ctx: KSBHHostCtxHandle { inner: ctx_handle },
            config: KSBHSliceKVStrings {
                ptr: config_entries.as_ptr(),
                len: config_entries.len(),
            },
            headers: KSBHSliceKVStrings {
                ptr: header_entries.as_ptr(),
                len: header_entries.len(),
            },
            request_info: &request_info,
            body,
            cookie_header,
            reputation_key: KSBHBytes::from_slice(&input.observed.client.reputation_key),
            internal_path: KSBHString::from_str(input.internal_path),
            session_id: SessionID {
                inner: *input.observed.session_id.as_bytes(),
            },
            h_log_fn: host_fn_log,
            h_free_bytes: host_fn_free_bytes,
            h_reputation_good_boy_fn: host_fn_reputation_good_boy,
            h_reputation_get_score_fn: host_fn_reputation_get_score,
            h_signal_get_fn: host_fn_signal_get,
            h_session_get_fn: host_fn_session_get_per_module,
            h_session_set_fn: host_fn_session_set_per_module,
            h_session_shared_get_fn: host_fn_session_get_shared,
            h_session_shared_set_fn: host_fn_session_set_shared,
        };

        let response = module.call_request_filter(input.stage, &mut ctx);
        let parsed = if response.is_null() {
            Ok(ModuleCallOutcome::Pass)
        } else {
            let parsed = parse_module_response(input.module_name, unsafe { &*response });
            unsafe {
                module.free_response(
                    &mut ctx as *mut ModuleContext,
                    response as *mut ModuleResponse,
                );
            }
            parsed
        };

        active_call_guard.finish();

        parsed
    }
}

fn build_header_entries(headers: &http::HeaderMap) -> Vec<KSBHKVString> {
    headers
        .iter()
        .map(|(name, value)| KSBHKVString {
            key: KSBHString::from_str(name.as_str()),
            value: bytes_to_string(value.as_bytes()),
        })
        .collect()
}

fn bytes_from_bytes(value: &bytes::Bytes) -> KSBHBytes {
    KSBHBytes {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

fn parse_module_response(
    module_name: &str,
    response: &ModuleResponse,
) -> Result<ModuleCallOutcome, ModuleHostError> {
    let body = if response.body.len == 0 {
        bytes::Bytes::new()
    } else if response.body.ptr.is_null() {
        return Err(ModuleHostError::InternalError(format!(
            "module {module_name} returned a response body with a null pointer"
        )));
    } else {
        unsafe {
            bytes::Bytes::copy_from_slice(::std::slice::from_raw_parts(
                response.body.ptr,
                response.body.len,
            ))
        }
    };

    match response.decision {
        KSBHModuleDecision::Pass => Ok(ModuleCallOutcome::Pass),
        KSBHModuleDecision::Stop => {
            let status = http::StatusCode::from_u16(response.status_code).map_err(|error| {
                ModuleHostError::InternalError(format!(
                    "module {module_name} returned invalid status code {}: {}",
                    response.status_code, error
                ))
            })?;

            let mut http_response = http::Response::builder().status(status);
            for entry in response_headers(response)? {
                http_response = http_response.header(entry.0, entry.1);
            }

            let response = http_response
                .body(body)
                .map_err(|error| ModuleHostError::InternalError(error.to_string()))?;

            Ok(ModuleCallOutcome::Stop(response))
        }
        KSBHModuleDecision::Error => {
            let message = if body.is_empty() {
                format!("module {module_name} returned an error")
            } else {
                String::from_utf8_lossy(&body).into_owned()
            };
            Ok(ModuleCallOutcome::Error(message))
        }
    }
}

fn response_headers(
    response: &ModuleResponse,
) -> Result<Vec<(http::header::HeaderName, http::HeaderValue)>, ModuleHostError> {
    if response.headers.ptr.is_null() || response.headers.len == 0 {
        return Ok(Vec::new());
    }

    let headers =
        unsafe { ::std::slice::from_raw_parts(response.headers.ptr, response.headers.len) };

    let mut out = Vec::with_capacity(headers.len());
    for header in headers {
        let Ok(name) = http::header::HeaderName::from_bytes(header.key.inner.as_slice())
        else {
            continue;
        };
        let Ok(value) = http::HeaderValue::from_bytes(header.value.inner.as_slice())
        else {
            continue;
        };

        out.push((name, value));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::active_call::{ACTIVE_CALLS, ActiveCallGuard, ActiveModuleCall};
    use crate::modules::runtime::host_functions::{
        host_fn_reputation_get_score, host_fn_reputation_good_boy, host_fn_signal_get,
    };
    use ksbh_modules_abi::functions::KSBHHostSignalKind;
    use ksbh_modules_abi::prelude::KSBHHostFnReturn;

    #[test]
    fn reputation_callbacks_use_active_call_identity_and_ip_buckets() {
        let store = ::std::sync::Arc::new(crate::storage::redis_hashmap::RedisHashMap::new(
            None, None, None,
        ));
        let reputation_key = [11u8; 32];
        let client_ip = Some(::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(
            192, 0, 2, 12,
        )));
        let active_call = ::std::sync::Arc::new(ActiveModuleCall::new(
            "test-module".to_string(),
            *uuid::Uuid::nil().as_bytes(),
            reputation_key,
            client_ip,
            store.clone(),
        ));
        let ctx_handle = ACTIVE_CALLS.insert(active_call);
        let mut guard = ActiveCallGuard::new(ctx_handle);

        store.reputation_observe(reputation_key, client_ip, 25);

        let mut score = 0u64;
        let result = unsafe {
            host_fn_reputation_get_score(KSBHHostCtxHandle { inner: ctx_handle }, &mut score)
        };
        assert_eq!(result, KSBHHostFnReturn::Success);
        assert_eq!(score, 25);

        store.reputation_observe(reputation_key, None, 40);
        let result =
            unsafe { host_fn_reputation_good_boy(KSBHHostCtxHandle { inner: ctx_handle }) };
        assert_eq!(result, KSBHHostFnReturn::Success);

        let result = unsafe {
            host_fn_reputation_get_score(KSBHHostCtxHandle { inner: ctx_handle }, &mut score)
        };
        assert_eq!(result, KSBHHostFnReturn::Success);
        assert_eq!(score, 25);

        guard.finish();
    }

    #[test]
    fn signal_callbacks_read_live_runtime_state() {
        let store = ::std::sync::Arc::new(crate::storage::redis_hashmap::RedisHashMap::new(
            None, None, None,
        ));
        let active_call = ::std::sync::Arc::new(ActiveModuleCall::new(
            "test-module".to_string(),
            *uuid::Uuid::nil().as_bytes(),
            [0u8; 32],
            None,
            store.clone(),
        ));
        let ctx_handle = ACTIVE_CALLS.insert(active_call);
        let mut guard = ActiveCallGuard::new(ctx_handle);

        struct RuntimeSignalGuard;
        impl Drop for RuntimeSignalGuard {
            fn drop(&mut self) {
                crate::metrics::runtime_signals::RUNTIME_SIGNALS.request_finished();
            }
        }

        crate::metrics::runtime_signals::RUNTIME_SIGNALS.request_started();
        let _runtime_signal_guard = RuntimeSignalGuard;
        crate::metrics::runtime_signals::RUNTIME_SIGNALS
            .observe_completion(http::StatusCode::INTERNAL_SERVER_ERROR);

        let mut signal = 0u64;
        let result = unsafe {
            host_fn_signal_get(
                KSBHHostCtxHandle { inner: ctx_handle },
                KSBHHostSignalKind::InFlightRequests,
                &mut signal,
            )
        };
        assert_eq!(result, KSBHHostFnReturn::Success);
        assert_eq!(signal, 1);

        let result = unsafe {
            host_fn_signal_get(
                KSBHHostCtxHandle { inner: ctx_handle },
                KSBHHostSignalKind::RecentErrorRateBps,
                &mut signal,
            )
        };
        assert_eq!(result, KSBHHostFnReturn::Success);
        assert_eq!(signal, 10_000);

        guard.finish();
    }
}
