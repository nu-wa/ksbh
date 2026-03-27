//! ModuleRequestContext and ModuleCallParams for efficient module calling.
//!
//! These structures are designed to reduce allocations in the hot path by
//! pre-computing request data once and sharing it across all module calls.

use super::ModuleKvSlice;
use super::module_request_info::RequestInfo;
use crate::modules::ModuleConfigurationType;

/// Pre-computed request data shared across all modules in the chain.
///
/// Built ONCE per request in request_filter, then passed to each module call.
/// This eliminates repeated header copying, query param parsing, and conversions.
pub struct ModuleRequestContext<'req> {
    /// HTTP headers as FFI-compatible slices (pointers into original headers)
    pub headers: Vec<ModuleKvSlice>,

    /// Pre-built RequestInfo for FFI
    pub request_info: RequestInfo,

    /// Cookie header (owned smol_str::SmolStr - no allocation for short strings)
    pub cookie_header: smol_str::SmolStr,

    /// Whether the host must append the proxy session cookie for module responses.
    pub needs_session_cookie: bool,

    /// Session ID as bytes (pre-converted from UUID)
    pub session_id_bytes: [u8; 16],

    /// Metrics key bytes
    pub metrics_key: &'req [u8],

    /// Request body (owned - allows reading multiple times if needed)
    pub body: Option<bytes::Bytes>,

    /// Internal path for module internal endpoints (e.g., /_ksbh_internal/)
    pub internal_path: smol_str::SmolStr,
}

impl<'req> ModuleRequestContext<'req> {
    /// Create a new ModuleRequestContext from session and request data.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        http_request: &ksbh_types::requests::http_request::HttpRequest,
        is_websocket_handshake: bool,
        needs_session_cookie: bool,
        session_id_bytes: [u8; 16],
        metrics_key: &'req [u8],
        body: Option<bytes::Bytes>,
        internal_path: &str,
    ) -> Self {
        let mut headers_vec = Vec::with_capacity(32);
        for (name, value) in session.header_map().iter() {
            headers_vec.push(ModuleKvSlice {
                key: bytes::Bytes::copy_from_slice(name.as_str().as_bytes()),
                value: bytes::Bytes::copy_from_slice(value.as_bytes()),
            });
        }

        let mut query_params_vec = Vec::with_capacity(16);
        for (k, v) in http_request.query.params.iter() {
            let k_ref: &str = k.as_ref();
            let v_ref: &str = v.as_ref();
            query_params_vec.push(ModuleKvSlice {
                key: bytes::Bytes::copy_from_slice(k_ref.as_bytes()),
                value: bytes::Bytes::copy_from_slice(v_ref.as_bytes()),
            });
        }

        let request_info =
            RequestInfo::new_owned(http_request, query_params_vec, is_websocket_handshake);

        let cookie_header = session
            .header_map()
            .get(http::header::COOKIE)
            .and_then(|c| c.to_str().ok())
            .map(smol_str::SmolStr::new)
            .unwrap_or_default();

        Self {
            headers: headers_vec,
            request_info,
            cookie_header,
            needs_session_cookie,
            session_id_bytes,
            metrics_key,
            body,
            internal_path: smol_str::SmolStr::new(internal_path),
        }
    }
}

/// Per-module parameters that don't reference request data.
///
/// Arc-wrapped to allow cheap cloning when passing to multiple modules.
pub struct ModuleCallParams {
    pub module_type: ModuleConfigurationType,
    pub config_kv_slice: ::std::sync::Arc<Vec<ModuleKvSlice>>,
    pub module_name: ::std::sync::Arc<ksbh_types::KsbhStr>,
}

impl ModuleCallParams {
    pub fn new(
        module_type: ModuleConfigurationType,
        config_kv_slice: ::std::sync::Arc<Vec<ModuleKvSlice>>,
        module_name: ::std::sync::Arc<ksbh_types::KsbhStr>,
    ) -> Self {
        Self {
            module_type,
            config_kv_slice,
            module_name,
        }
    }
}
