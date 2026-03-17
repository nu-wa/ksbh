fn parse_buffer(buffer: &ksbh_core::modules::abi::ModuleBuffer) -> smol_str::SmolStr {
    buffer.as_str().unwrap_or_default().into()
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
    }
}

#[repr(C)]
pub struct OwnedResponse {
    response: ksbh_core::modules::abi::ModuleResponse,
    headers: Vec<ksbh_core::modules::abi::ModuleKvSlice>, // Keep alive
}

impl Drop for OwnedResponse {
    fn drop(&mut self) {}
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

    let owned_ptr = Box::into_raw(Box::new(owned));
    owned_ptr as *const ksbh_core::modules::abi::ModuleResponse
}
