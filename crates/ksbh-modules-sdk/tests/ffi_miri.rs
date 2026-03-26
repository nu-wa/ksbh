fn ffi_smoke_process(
    ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    if ctx.request.path == "/pass" {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    if ctx.request.path == "/error/bad_request" {
        return Err(ksbh_modules_sdk::ModuleError::bad_request(
            "sdk ffi error path",
        ));
    }

    if ctx.request.path == "/error/unauthorized" {
        return Err(ksbh_modules_sdk::ModuleError::unauthorized(
            "sdk ffi unauthorized path",
        ));
    }

    if ctx.request.path == "/error/forbidden" {
        return Err(ksbh_modules_sdk::ModuleError::forbidden(
            "sdk ffi forbidden path",
        ));
    }

    if ctx.request.path == "/error/not_found" {
        return Err(ksbh_modules_sdk::ModuleError::not_found(
            "sdk ffi not found path",
        ));
    }

    if ctx.request.path == "/error/too_many" {
        return Err(ksbh_modules_sdk::ModuleError::too_many_requests(
            "sdk ffi too many requests path",
        ));
    }

    if ctx.request.path == "/error/internal" {
        return Err(ksbh_modules_sdk::ModuleError::internal_error(
            "sdk ffi internal error path",
        ));
    }

    if ctx.request.path == "/error/critical" {
        return Err(ksbh_modules_sdk::ModuleError::critical(
            ::std::io::Error::other("sdk ffi critical path"),
        ));
    }

    if ctx.request.path == "/panic" {
        panic!("ffi panic route");
    }

    let header_value = ctx
        .headers
        .get("x-test-header")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let config_value = ctx
        .config
        .get("greeting")
        .map(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let query_value = ctx
        .request
        .query_params
        .get("name")
        .map(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let session_value = ctx
        .session
        .get("persisted")
        .and_then(|value| ::std::string::String::from_utf8(value).ok())
        .unwrap_or_default();

    if !ctx.session.set("written", b"ok") {
        return Err(ksbh_modules_sdk::ModuleError::internal_error(
            "failed to write session value",
        ));
    }

    if !ctx.session.set_with_ttl("ttl-key", b"ttl", 42) {
        return Err(ksbh_modules_sdk::ModuleError::internal_error(
            "failed to write ttl session value",
        ));
    }

    let score = ctx.metrics.get_score(ctx.metrics_key);
    if !ctx.metrics.good_boy(ctx.metrics_key) {
        return Err(ksbh_modules_sdk::ModuleError::internal_error(
            "failed to mark good boy",
        ));
    }

    ksbh_modules_sdk::log_info!(
        ctx.logger,
        "ffi-smoke {} {} {}",
        ctx.request.method,
        ctx.request.path,
        score
    );

    let body = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        ctx.request.method,
        ctx.request.path,
        header_value,
        config_value,
        query_value,
        session_value,
        ctx.cookie_header,
        score
    );

    let response = http::Response::builder()
        .status(http::StatusCode::CREATED)
        .header("x-sdk-ffi", "ok")
        .body(bytes::Bytes::from(body))
        .map_err(|error| ksbh_modules_sdk::ModuleError::internal_error(error.to_string()))?;

    Ok(ksbh_modules_sdk::ModuleResult::Stop(response))
}

ksbh_modules_sdk::register_module!(
    ffi_smoke_process,
    ksbh_modules_sdk::types::ModuleType::Custom("ffi-miri-smoke".into())
);

type SessionSetCalls =
    ::std::sync::Mutex<::std::vec::Vec<(::std::string::String, ::std::vec::Vec<u8>)>>;
type SessionSetWithTtlCalls =
    ::std::sync::Mutex<::std::vec::Vec<(::std::string::String, ::std::vec::Vec<u8>, u64)>>;

static SESSION_SET_CALLS: ::std::sync::OnceLock<SessionSetCalls> = ::std::sync::OnceLock::new();
static SESSION_SET_TTL_CALLS: ::std::sync::OnceLock<SessionSetWithTtlCalls> =
    ::std::sync::OnceLock::new();
static GOOD_BOY_CALLS: ::std::sync::OnceLock<
    ::std::sync::Mutex<::std::vec::Vec<::std::vec::Vec<u8>>>,
> = ::std::sync::OnceLock::new();
static LOG_MESSAGES: ::std::sync::OnceLock<
    ::std::sync::Mutex<::std::vec::Vec<::std::string::String>>,
> = ::std::sync::OnceLock::new();

#[test]
fn exported_sdk_module_ffi_smoke_test() {
    reset_records();
    let query_params = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new("name", "codex")];
    let config = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new(
        "greeting", "hello",
    )];
    let headers = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new(
        "x-test-header",
        "header-value",
    )];
    let request_info = build_request_info("POST", "/ffi", &query_params);
    let metrics_key = default_metrics_key();
    let module_context = build_module_context(
        &config,
        &headers,
        &request_info,
        metrics_key,
        "ffi-miri-smoke",
    );

    let module_type = unsafe { get_module_type() };
    assert_eq!(
        module_type.code,
        ksbh_core::modules::abi::ModuleTypeCode::Custom
    );
    let type_name = unsafe {
        ::std::str::from_utf8(::std::slice::from_raw_parts(
            module_type.custom_ptr,
            module_type.custom_len,
        ))
        .unwrap_or_default()
        .to_string()
    };
    assert_eq!(type_name, "ffi-miri-smoke");

    let response_ptr = unsafe { request_filter(&module_context) };
    assert!(!response_ptr.is_null());

    let response = unsafe { &*response_ptr };
    assert_eq!(response.status_code, http::StatusCode::CREATED.as_u16());

    let body = ::std::str::from_utf8(response.body.as_ref())
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        body,
        "POST|/ffi|header-value|hello|codex|from-host|ksbh=session-cookie|250"
    );

    let headers = response.headers_slice();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].key_str(), "x-sdk-ffi");
    assert_eq!(headers[0].value_str(), "ok");

    let set_calls = session_set_calls();
    let set_calls = set_calls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(set_calls.len(), 1);
    assert_eq!(set_calls[0].0, "written");
    assert_eq!(set_calls[0].1, b"ok");
    drop(set_calls);

    let ttl_calls = session_set_ttl_calls();
    let ttl_calls = ttl_calls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(ttl_calls.len(), 1);
    assert_eq!(ttl_calls[0].0, "ttl-key");
    assert_eq!(ttl_calls[0].1, b"ttl");
    assert_eq!(ttl_calls[0].2, 42);
    drop(ttl_calls);

    let good_boy_calls = good_boy_calls();
    let good_boy_calls = good_boy_calls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(good_boy_calls.len(), 1);
    assert_eq!(
        good_boy_calls[0],
        ::std::vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    drop(good_boy_calls);

    let logs = log_messages();
    let logs = logs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(logs.len(), 1);
    assert!(logs[0].contains("ffi-smoke POST /ffi 250"));
    drop(logs);

    free_response_from_module_ptr(response_ptr);
}

#[test]
fn exported_sdk_module_custom_type_pointer_is_stable() {
    let first = unsafe { get_module_type() };
    let second = unsafe { get_module_type() };

    assert_eq!(first.code, ksbh_core::modules::abi::ModuleTypeCode::Custom);
    assert_eq!(second.code, ksbh_core::modules::abi::ModuleTypeCode::Custom);
    assert_eq!(first.custom_ptr, second.custom_ptr);
    assert_eq!(first.custom_len, second.custom_len);
}

#[test]
fn exported_sdk_module_pass_returns_null_response_pointer() {
    let query_params = ::std::vec::Vec::new();
    let config = ::std::vec::Vec::new();
    let headers = ::std::vec::Vec::new();
    let request_info = build_request_info("GET", "/pass", &query_params);
    let metrics_key = default_metrics_key();
    let module_context = build_module_context(
        &config,
        &headers,
        &request_info,
        metrics_key,
        "ffi-miri-smoke",
    );

    let response_ptr = unsafe { request_filter(&module_context) };
    assert!(response_ptr.is_null());
}

#[test]
fn request_filter_null_ctx_returns_null() {
    let response_ptr = unsafe { request_filter(::std::ptr::null()) };
    assert!(response_ptr.is_null());
}

#[test]
fn module_error_bad_request_maps_to_400() {
    assert_error_path(
        "/error/bad_request",
        http::StatusCode::BAD_REQUEST,
        "sdk ffi error path",
    );
}

#[test]
fn module_error_unauthorized_maps_to_401() {
    assert_error_path(
        "/error/unauthorized",
        http::StatusCode::UNAUTHORIZED,
        "sdk ffi unauthorized path",
    );
}

#[test]
fn module_error_forbidden_maps_to_403() {
    assert_error_path(
        "/error/forbidden",
        http::StatusCode::FORBIDDEN,
        "sdk ffi forbidden path",
    );
}

#[test]
fn module_error_not_found_maps_to_404() {
    assert_error_path(
        "/error/not_found",
        http::StatusCode::NOT_FOUND,
        "sdk ffi not found path",
    );
}

#[test]
fn module_error_too_many_requests_maps_to_429() {
    assert_error_path(
        "/error/too_many",
        http::StatusCode::TOO_MANY_REQUESTS,
        "sdk ffi too many requests path",
    );
}

#[test]
fn module_error_internal_error_maps_to_500() {
    assert_error_path(
        "/error/internal",
        http::StatusCode::INTERNAL_SERVER_ERROR,
        "sdk ffi internal error path",
    );
}

#[test]
fn module_error_critical_maps_to_500_with_critical_prefix() {
    assert_error_path(
        "/error/critical",
        http::StatusCode::INTERNAL_SERVER_ERROR,
        "Critical: sdk ffi critical path",
    );
}

#[test]
fn panic_in_handler_maps_to_500_module_panic_body() {
    assert_error_path(
        "/panic",
        http::StatusCode::INTERNAL_SERVER_ERROR,
        "Module panic",
    );
}

#[test]
fn response_free_via_exported_free_response_single_round_trip() {
    let query_params = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new("name", "codex")];
    let config = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new(
        "greeting", "hello",
    )];
    let headers = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new(
        "x-test-header",
        "header-value",
    )];
    let request_info = build_request_info("POST", "/ffi", &query_params);
    let metrics_key = default_metrics_key();
    let module_context = build_module_context(
        &config,
        &headers,
        &request_info,
        metrics_key,
        "ffi-miri-smoke",
    );

    let response_ptr = unsafe { request_filter(&module_context) };
    assert!(!response_ptr.is_null());
    let response = unsafe { &*response_ptr };
    assert_eq!(response.status_code, http::StatusCode::CREATED.as_u16());
    free_response_from_module_ptr(response_ptr);
}

#[test]
fn free_response_null_and_zero_args_is_safe_noop() {
    unsafe {
        ksbh_modules_sdk::free_response(::std::ptr::null(), 0, ::std::ptr::null(), 0);
    }
}

#[test]
fn response_free_via_exported_free_response_repeated_loop() {
    reset_records();

    let query_params = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new("name", "codex")];
    let config = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new(
        "greeting", "hello",
    )];
    let headers = ::std::vec![ksbh_core::modules::abi::ModuleKvSlice::new(
        "x-test-header",
        "header-value",
    )];
    let request_info = build_request_info("POST", "/ffi", &query_params);
    let metrics_key = default_metrics_key();
    let module_context = build_module_context(
        &config,
        &headers,
        &request_info,
        metrics_key,
        "ffi-miri-smoke",
    );

    for _ in 0..64 {
        let response_ptr = unsafe { request_filter(&module_context) };
        assert!(!response_ptr.is_null());

        let response = unsafe { &*response_ptr };
        assert_eq!(response.status_code, http::StatusCode::CREATED.as_u16());
        free_response_from_module_ptr(response_ptr);
    }
}

fn default_metrics_key() -> ksbh_core::modules::abi::ModuleBuffer {
    ksbh_core::modules::abi::ModuleBuffer::from_ref_bytes(&[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    ])
}

fn free_response_from_module_ptr(response_ptr: *const ksbh_core::modules::abi::ModuleResponse) {
    if response_ptr.is_null() {
        return;
    }

    let response = unsafe { &*response_ptr };
    unsafe {
        ksbh_modules_sdk::free_response(
            response.headers_ptr,
            response.headers_len,
            response.body.as_ptr(),
            response.body.len(),
        );
    }
}

fn assert_error_path(path: &str, expected_status: http::StatusCode, expected_body: &str) {
    let query_params = ::std::vec::Vec::new();
    let config = ::std::vec::Vec::new();
    let headers = ::std::vec::Vec::new();
    let request_info = build_request_info("GET", path, &query_params);
    let metrics_key = default_metrics_key();
    let module_context = build_module_context(
        &config,
        &headers,
        &request_info,
        metrics_key,
        "ffi-miri-smoke",
    );

    let response_ptr = unsafe { request_filter(&module_context) };
    assert!(!response_ptr.is_null());

    let response = unsafe { &*response_ptr };
    assert_eq!(response.status_code, expected_status.as_u16());
    assert_eq!(
        ::std::str::from_utf8(response.body.as_ref()).unwrap_or_default(),
        expected_body
    );
    free_response_from_module_ptr(response_ptr);
}

fn build_request_info(
    method: &str,
    path: &str,
    query_params: &[ksbh_core::modules::abi::ModuleKvSlice],
) -> ksbh_core::modules::abi::RequestInfo {
    let uri = if query_params.is_empty() {
        format!("http://example.test{}", path)
    } else {
        format!("http://example.test{}?name=codex", path)
    };

    ksbh_core::modules::abi::RequestInfo {
        uri: ksbh_core::modules::abi::ModuleBuffer::from_ref(&uri),
        host: ksbh_core::modules::abi::ModuleBuffer::from_ref("example.test"),
        method: ksbh_core::modules::abi::ModuleBuffer::from_ref(method),
        path: ksbh_core::modules::abi::ModuleBuffer::from_ref(path),
        query_params: ksbh_core::modules::abi::QueryParams::new(query_params),
        scheme: ksbh_core::modules::abi::ModuleBuffer::from_ref("http"),
        port: 8080,
        is_websocket_handshake: false,
    }
}

fn build_module_context<'a>(
    config: &'a [ksbh_core::modules::abi::ModuleKvSlice],
    headers: &'a [ksbh_core::modules::abi::ModuleKvSlice],
    request_info: &'a ksbh_core::modules::abi::RequestInfo,
    metrics_key: ksbh_core::modules::abi::ModuleBuffer,
    mod_name: &'a str,
) -> ksbh_core::modules::abi::ModuleContext<'a> {
    ksbh_core::modules::abi::ModuleContext {
        config,
        headers,
        req_info: request_info,
        body: b"request-body",
        log_fn: mock_log,
        session_id: [9u8; 16],
        session_get_fn: mock_session_get,
        session_set_fn: mock_session_set,
        session_set_with_ttl_fn: mock_session_set_with_ttl,
        session_free_fn: mock_session_free,
        mod_name,
        cookie_header: ksbh_core::modules::abi::ModuleBuffer::from_ref("ksbh=session-cookie"),
        metrics_key,
        metrics_good_boy_fn: mock_metrics_good_boy,
        metrics_get_score_fn: mock_metrics_get_score,
        internal_path: ksbh_core::modules::abi::ModuleBuffer::from_ref("/_ksbh_internal/"),
    }
}

fn session_set_calls()
-> &'static ::std::sync::Mutex<::std::vec::Vec<(::std::string::String, ::std::vec::Vec<u8>)>> {
    SESSION_SET_CALLS.get_or_init(|| ::std::sync::Mutex::new(::std::vec::Vec::new()))
}

fn session_set_ttl_calls()
-> &'static ::std::sync::Mutex<::std::vec::Vec<(::std::string::String, ::std::vec::Vec<u8>, u64)>> {
    SESSION_SET_TTL_CALLS.get_or_init(|| ::std::sync::Mutex::new(::std::vec::Vec::new()))
}

fn good_boy_calls() -> &'static ::std::sync::Mutex<::std::vec::Vec<::std::vec::Vec<u8>>> {
    GOOD_BOY_CALLS.get_or_init(|| ::std::sync::Mutex::new(::std::vec::Vec::new()))
}

fn log_messages() -> &'static ::std::sync::Mutex<::std::vec::Vec<::std::string::String>> {
    LOG_MESSAGES.get_or_init(|| ::std::sync::Mutex::new(::std::vec::Vec::new()))
}

fn reset_records() {
    session_set_calls()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    session_set_ttl_calls()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    good_boy_calls()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    log_messages()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

unsafe extern "C" fn mock_log(
    _level: u8,
    _target: *const u8,
    _target_len: usize,
    message: *const u8,
    message_len: usize,
) -> u8 {
    let message_slice = unsafe { ::std::slice::from_raw_parts(message, message_len) };
    let message_value = ::std::str::from_utf8(message_slice)
        .unwrap_or_default()
        .to_string();
    log_messages()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(message_value);
    0
}

unsafe extern "C" fn mock_session_get(
    _session_id: *const u8,
    _module_name: *const u8,
    _module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> bool {
    let key_slice = unsafe { ::std::slice::from_raw_parts(data_key, data_key_len) };
    let key = ::std::str::from_utf8(key_slice).unwrap_or_default();

    if key != "persisted" {
        return false;
    }

    let data = b"from-host".to_vec().into_boxed_slice();
    let len = data.len();
    let ptr = Box::into_raw(data) as *const u8;

    unsafe {
        *out_ptr = ptr;
        *out_len = len;
    }

    true
}

unsafe extern "C" fn mock_session_set(
    _session_id: *const u8,
    _module_name: *const u8,
    _module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    data_ptr: *const u8,
    data_len: usize,
) -> bool {
    let key_slice = unsafe { ::std::slice::from_raw_parts(data_key, data_key_len) };
    let data_slice = unsafe { ::std::slice::from_raw_parts(data_ptr, data_len) };

    session_set_calls()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push((
            ::std::str::from_utf8(key_slice)
                .unwrap_or_default()
                .to_string(),
            data_slice.to_vec(),
        ));

    true
}

unsafe extern "C" fn mock_session_set_with_ttl(
    _session_id: *const u8,
    _module_name: *const u8,
    _module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    data_ptr: *const u8,
    data_len: usize,
    ttl_secs: u64,
) -> bool {
    let key_slice = unsafe { ::std::slice::from_raw_parts(data_key, data_key_len) };
    let data_slice = unsafe { ::std::slice::from_raw_parts(data_ptr, data_len) };

    session_set_ttl_calls()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push((
            ::std::str::from_utf8(key_slice)
                .unwrap_or_default()
                .to_string(),
            data_slice.to_vec(),
            ttl_secs,
        ));

    true
}

unsafe extern "C" fn mock_session_free(
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
        drop(Box::from_raw(slice));
    }
}

unsafe extern "C" fn mock_metrics_good_boy(metrics_key: *const u8, metrics_key_len: usize) -> bool {
    let key = unsafe { ::std::slice::from_raw_parts(metrics_key, metrics_key_len) };
    good_boy_calls()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(key.to_vec());
    true
}

unsafe extern "C" fn mock_metrics_get_score(
    _metrics_key: *const u8,
    _metrics_key_len: usize,
) -> u64 {
    250
}
