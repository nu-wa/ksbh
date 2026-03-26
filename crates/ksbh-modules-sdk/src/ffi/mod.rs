fn parse_buffer(buffer: &ksbh_core::modules::abi::ModuleBuffer) -> smol_str::SmolStr {
    buffer.as_str().unwrap_or_default().into()
}

type ResponseOwnerKey = (usize, usize, usize, usize);
type ResponseOwners =
    ::std::sync::Mutex<::std::collections::HashMap<ResponseOwnerKey, OwnedResponsePtr>>;

#[derive(Clone, Copy)]
struct OwnedResponsePtr(*mut OwnedResponse);

// SAFETY: pointers are only inserted/removed behind `ResponseOwners` mutex, and ownership
// transfer is explicit via `Box::into_raw`/`Box::from_raw` in this module.
unsafe impl Send for OwnedResponsePtr {}

// SAFETY: same reasoning as `Send`; synchronization is provided by the registry mutex.
unsafe impl Sync for OwnedResponsePtr {}

fn response_owners() -> &'static ResponseOwners {
    static RESPONSE_OWNERS: ::std::sync::OnceLock<ResponseOwners> = ::std::sync::OnceLock::new();
    RESPONSE_OWNERS.get_or_init(|| ::std::sync::Mutex::new(::std::collections::HashMap::new()))
}

pub fn free_owned_response_by_parts(
    headers_ptr: *const ksbh_core::modules::abi::ModuleKvSlice,
    headers_len: usize,
    body_ptr: *const u8,
    body_len: usize,
) -> bool {
    let key = (
        headers_ptr as usize,
        headers_len,
        body_ptr as usize,
        body_len,
    );
    let removed = response_owners()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&key);
    if let Some(owned) = removed {
        // SAFETY: `owned.0` was created by `Box::into_raw` in `alloc_response`, and is removed
        // exactly once here before converting back into a `Box` for drop.
        unsafe {
            drop(Box::from_raw(owned.0));
        }
        true
    } else {
        false
    }
}

pub fn convert_context<'a>(
    ffi_ctx: &'a ksbh_core::modules::abi::ModuleContext<'a>,
) -> crate::context::RequestContext<'a> {
    let config = parse_config(ffi_ctx.config);
    let headers = parse_headers(ffi_ctx.headers);
    let request = parse_request_info(ffi_ctx.req_info);
    let session = crate::session::SessionHandle::from_ffi(
        ffi_ctx.session_id,
        smol_str::SmolStr::new(ffi_ctx.mod_name),
        ffi_ctx.session_get_fn,
        ffi_ctx.session_set_fn,
        ffi_ctx.session_set_with_ttl_fn,
        ffi_ctx.session_free_fn,
    );
    let logger = crate::logger::Logger::new(ffi_ctx.log_fn, ffi_ctx.mod_name);

    let metrics = crate::metrics::MetricsHandle::from_ffi(
        ffi_ctx.metrics_good_boy_fn,
        ffi_ctx.metrics_get_score_fn,
    );

    crate::context::RequestContext {
        config,
        headers,
        request,
        body: ffi_ctx.body,
        session,
        logger,
        mod_name: smol_str::SmolStr::new(ffi_ctx.mod_name),
        metrics_key: ffi_ctx.metrics_key.as_bytes(),
        cookie_header: parse_buffer(&ffi_ctx.cookie_header),
        metrics,
        internal_path: ffi_ctx.internal_path.as_str().unwrap_or(""),
    }
}

pub fn parse_config(
    config: &[ksbh_core::modules::abi::ModuleKvSlice],
) -> ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr> {
    let mut map = ::std::collections::HashMap::new();

    for kv in config {
        let key = ::std::str::from_utf8(&kv.key);
        let value = ::std::str::from_utf8(&kv.value);

        if let (Ok(k), Ok(v)) = (key, value) {
            map.insert(k.into(), v.into());
        }
    }

    map
}

pub fn parse_headers(headers: &[ksbh_core::modules::abi::ModuleKvSlice]) -> http::HeaderMap {
    let mut map = http::HeaderMap::new();

    for kv in headers {
        let key = ::std::str::from_utf8(&kv.key);
        let value = ::std::str::from_utf8(&kv.value);

        if let (Ok(k), Ok(v)) = (key, value)
            && let (Ok(name), Ok(val)) = (
                k.parse::<http::header::HeaderName>(),
                v.parse::<http::header::HeaderValue>(),
            )
        {
            map.insert(name, val);
        }
    }
    map
}

pub fn parse_request_info(
    req_info: &ksbh_core::modules::abi::RequestInfo,
) -> crate::context::RequestInfo {
    let mut query_params = ::std::collections::HashMap::new();

    for kv in req_info.get_query_params() {
        let key = ::std::str::from_utf8(&kv.key);
        let value = ::std::str::from_utf8(&kv.value);
        if let (Ok(k), Ok(v)) = (key, value) {
            query_params.insert(k.into(), v.into());
        }
    }

    crate::context::RequestInfo {
        uri: req_info.get_uri().unwrap_or_default().into(),
        host: req_info.get_host().unwrap_or_default().into(),
        method: req_info.get_method().unwrap_or_default().into(),
        path: req_info.get_path().unwrap_or_default().into(),
        query_params,
        scheme: req_info.get_scheme().unwrap_or_default().into(),
        port: req_info.get_port(),
        is_websocket_handshake: req_info.is_websocket_handshake(),
    }
}

#[repr(C)]
pub struct OwnedResponse {
    response: ksbh_core::modules::abi::ModuleResponse,
    headers: Vec<ksbh_core::modules::abi::ModuleKvSlice>, // Keep alive
}

impl Drop for OwnedResponse {
    fn drop(&mut self) {
        let _ = self.headers.len();
    }
}

pub fn alloc_response(
    response: http::Response<bytes::Bytes>,
) -> *const ksbh_core::modules::abi::ModuleResponse {
    let (parts, body) = response.into_parts();

    let headers_vec: Vec<ksbh_core::modules::abi::ModuleKvSlice> = parts
        .headers
        .iter()
        .map(|(name, value)| ksbh_core::modules::abi::ModuleKvSlice {
            key: bytes::Bytes::copy_from_slice(name.as_str().as_bytes()),
            value: bytes::Bytes::copy_from_slice(value.as_bytes()),
        })
        .collect();

    let headers_ptr = headers_vec.as_ptr();
    let headers_len = headers_vec.len();

    let module_response = ksbh_core::modules::abi::module_response::ModuleResponse {
        status_code: parts.status.as_u16(),
        headers_ptr,
        headers_len,
        body,
    };

    // Keep headers alive by storing in OwnedResponse
    let owned = OwnedResponse {
        response: module_response,
        headers: headers_vec,
    };

    let body_ptr = owned.response.body.as_ptr();
    let body_len = owned.response.body.len();
    let key = (
        headers_ptr as usize,
        headers_len,
        body_ptr as usize,
        body_len,
    );
    let owned_ptr = Box::into_raw(Box::new(owned));
    let response_ptr = {
        // SAFETY: `owned_ptr` remains valid until explicitly freed through the response owner
        // registry via `free_owned_response_by_parts`.
        unsafe { &(*owned_ptr).response as *const ksbh_core::modules::abi::ModuleResponse }
    };

    match response_owners().lock() {
        Ok(mut registry) => {
            registry.insert(key, OwnedResponsePtr(owned_ptr));
        }
        Err(poisoned) => {
            let mut registry = poisoned.into_inner();
            registry.insert(key, OwnedResponsePtr(owned_ptr));
        }
    }

    response_ptr
}

#[cfg(test)]
mod tests {
    fn make_kv_slice(
        key: ::std::vec::Vec<u8>,
        value: ::std::vec::Vec<u8>,
    ) -> ksbh_core::modules::abi::ModuleKvSlice {
        ksbh_core::modules::abi::ModuleKvSlice {
            key: bytes::Bytes::from(key),
            value: bytes::Bytes::from(value),
        }
    }

    fn make_request_info(
        query_params: ::std::vec::Vec<ksbh_core::modules::abi::ModuleKvSlice>,
    ) -> ksbh_core::modules::abi::RequestInfo {
        ksbh_core::modules::abi::RequestInfo {
            uri: ksbh_core::modules::abi::ModuleBuffer::from_ref("http://example.test/path"),
            host: ksbh_core::modules::abi::ModuleBuffer::from_ref("example.test"),
            method: ksbh_core::modules::abi::ModuleBuffer::from_ref("GET"),
            path: ksbh_core::modules::abi::ModuleBuffer::from_ref("/path"),
            query_params: ksbh_core::modules::abi::QueryParams {
                params: query_params,
            },
            scheme: ksbh_core::modules::abi::ModuleBuffer::from_ref("http"),
            port: 80,
            is_websocket_handshake: false,
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_parse_headers_never_panics_and_only_keeps_valid_pairs(
            pairs in proptest::collection::vec(
                (
                    proptest::collection::vec(proptest::num::u8::ANY, 0..16),
                    proptest::collection::vec(proptest::num::u8::ANY, 0..32)
                ),
                0..32
            )
        ) {
            let slices: ::std::vec::Vec<ksbh_core::modules::abi::ModuleKvSlice> = pairs
                .iter()
                .map(|(k, v)| make_kv_slice(k.clone(), v.clone()))
                .collect();

            let parsed = super::parse_headers(&slices);
            for (k, v) in pairs {
                if let (Ok(key), Ok(value)) = (::std::str::from_utf8(&k), ::std::str::from_utf8(&v))
                    && let (Ok(name), Ok(header_value)) = (
                        key.parse::<http::header::HeaderName>(),
                        value.parse::<http::header::HeaderValue>(),
                    )
                    && let Some(parsed_value) = parsed.get(name)
                {
                    proptest::prop_assert_eq!(parsed_value, &header_value);
                }
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_parse_config_no_panic_invalid_utf8_filtered(
            pairs in proptest::collection::vec(
                (
                    proptest::collection::vec(proptest::num::u8::ANY, 0..16),
                    proptest::collection::vec(proptest::num::u8::ANY, 0..32)
                ),
                0..32
            )
        ) {
            let slices: ::std::vec::Vec<ksbh_core::modules::abi::ModuleKvSlice> = pairs
                .iter()
                .map(|(k, v)| make_kv_slice(k.clone(), v.clone()))
                .collect();

            let parsed = super::parse_config(&slices);
            let mut expected = ::std::collections::HashMap::<::std::string::String, ::std::string::String>::new();
            for (k, v) in pairs {
                if let (Ok(key), Ok(value)) = (::std::str::from_utf8(&k), ::std::str::from_utf8(&v)) {
                    expected.insert(key.to_string(), value.to_string());
                }
            }

            for (key, value) in expected {
                if let Some(parsed_value) = parsed.get(key.as_str()) {
                    proptest::prop_assert_eq!(parsed_value.as_str(), value.as_str());
                } else {
                    proptest::prop_assert!(false, "missing expected config key");
                }
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_parse_request_info_no_panic_and_query_semantics_stable(
            pairs in proptest::collection::vec(
                (
                    proptest::collection::vec(proptest::num::u8::ANY, 0..16),
                    proptest::collection::vec(proptest::num::u8::ANY, 0..32)
                ),
                0..32
            )
        ) {
            let query_params: ::std::vec::Vec<ksbh_core::modules::abi::ModuleKvSlice> = pairs
                .iter()
                .map(|(k, v)| make_kv_slice(k.clone(), v.clone()))
                .collect();
            let req_info = make_request_info(query_params);
            let parsed = super::parse_request_info(&req_info);

            let mut expected = ::std::collections::HashMap::<::std::string::String, ::std::string::String>::new();
            for (k, v) in pairs {
                if let (Ok(key), Ok(value)) = (::std::str::from_utf8(&k), ::std::str::from_utf8(&v)) {
                    expected.insert(key.to_string(), value.to_string());
                }
            }

            for (key, value) in expected {
                if let Some(parsed_value) = parsed.query_params.get(key.as_str()) {
                    proptest::prop_assert_eq!(parsed_value.as_str(), value.as_str());
                } else {
                    proptest::prop_assert!(false, "missing expected query key");
                }
            }
        }
    }
}
