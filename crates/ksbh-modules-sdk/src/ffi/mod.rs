fn parse_buffer(buffer: &ksbh_core::modules::abi::ModuleBuffer) -> smol_str::SmolStr {
    buffer.as_str().unwrap_or_default().into()
}

pub fn convert_context<'a>(
    ffi_ctx: &ksbh_core::modules::abi::ModuleContext<'a>,
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

    let user_agent = parse_buffer(&ffi_ctx.user_agent);
    let user_agent = if user_agent.is_empty() {
        None
    } else {
        Some(user_agent)
    };

    let metrics = crate::metrics::MetricsHandle::from_ffi(
        ffi_ctx.metrics_increment_good_fn,
        ffi_ctx.metrics_get_hits_fn,
    );

    crate::context::RequestContext {
        config: Box::leak(Box::new(config)),
        headers: Box::leak(Box::new(headers)),
        request,
        body: ffi_ctx.body,
        session,
        logger,
        mod_name: smol_str::SmolStr::new(ffi_ctx.mod_name),
        client_ip: parse_buffer(&ffi_ctx.client_ip),
        user_agent,
        cookie_header: parse_buffer(&ffi_ctx.cookie_header),
        metrics,
    }
}

pub fn parse_config(
    config: &[ksbh_core::modules::abi::ModuleKvSlice],
) -> ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr> {
    let mut map = ::std::collections::HashMap::new();

    for kv in config {
        if kv.key.is_null() || kv.value.is_null() {
            continue;
        }

        let key =
            unsafe { ::std::str::from_utf8(::std::slice::from_raw_parts(kv.key, kv.key_len)) };
        let value =
            unsafe { ::std::str::from_utf8(::std::slice::from_raw_parts(kv.value, kv.value_len)) };

        if let (Ok(k), Ok(v)) = (key, value) {
            map.insert(k.into(), v.into());
        }
    }

    map
}

pub fn parse_headers(headers: &[ksbh_core::modules::abi::ModuleKvSlice]) -> http::HeaderMap {
    let mut map = http::HeaderMap::new();

    for kv in headers {
        if kv.key.is_null() || kv.value.is_null() {
            continue;
        }

        let key =
            unsafe { ::std::str::from_utf8(::std::slice::from_raw_parts(kv.key, kv.key_len)) };
        let value =
            unsafe { ::std::str::from_utf8(::std::slice::from_raw_parts(kv.value, kv.value_len)) };

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
        let key = unsafe { std::str::from_utf8(std::slice::from_raw_parts(kv.key, kv.key_len)) };
        let value =
            unsafe { std::str::from_utf8(std::slice::from_raw_parts(kv.value, kv.value_len)) };
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
    headers: Vec<u8>,
    body: Vec<u8>,
}

impl Drop for OwnedResponse {
    fn drop(&mut self) {}
}

pub fn alloc_response(
    response: http::Response<bytes::Bytes>,
) -> *const ksbh_core::modules::abi::ModuleResponse {
    let (parts, body) = response.into_parts();
    let body_vec = body.to_vec();
    let headers_bytes = serialize_headers(&parts.headers);

    let response = ksbh_core::modules::abi::module_response::ModuleResponse {
        status_code: parts.status.as_u16(),
        headers: headers_bytes.as_ptr(),
        headers_size: headers_bytes.len(),
        body: body_vec.as_ptr(),
        body_size: body_vec.len(),
    };

    let owned = OwnedResponse {
        response,
        headers: headers_bytes,
        body: body_vec,
    };

    let owned_ptr = Box::into_raw(Box::new(owned));
    owned_ptr as *const ksbh_core::modules::abi::ModuleResponse
}

pub fn serialize_headers(headers: &http::HeaderMap) -> Vec<u8> {
    let mut data = Vec::new();

    for (name, value) in headers {
        let name_b = name.as_str().as_bytes();
        let value_b = value.as_bytes();
        data.extend_from_slice(&(name_b.len() as u32).to_le_bytes());
        data.extend_from_slice(name_b);
        data.extend_from_slice(&(value_b.len() as u32).to_le_bytes());
        data.extend_from_slice(value_b);
    }

    data
}
