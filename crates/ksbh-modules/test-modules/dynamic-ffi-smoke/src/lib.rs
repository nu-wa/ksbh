#[repr(C)]
struct ResponseRecord {
    response: ksbh_core::modules::abi::ModuleResponse,
    headers: ::std::vec::Vec<ksbh_core::modules::abi::ModuleKvSlice>,
}

unsafe impl Send for ResponseRecord {}

static RESPONSE_RECORDS: ::std::sync::OnceLock<
    ::std::sync::Mutex<::std::vec::Vec<::std::boxed::Box<ResponseRecord>>>,
> = ::std::sync::OnceLock::new();

static FREE_RESPONSE_CALLS: ::std::sync::atomic::AtomicUsize =
    ::std::sync::atomic::AtomicUsize::new(0);
static LAST_OPERATION: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);

const OP_NONE: usize = 0;
const OP_FREE_COUNT: usize = 2;

fn response_records()
-> &'static ::std::sync::Mutex<::std::vec::Vec<::std::boxed::Box<ResponseRecord>>> {
    RESPONSE_RECORDS.get_or_init(|| ::std::sync::Mutex::new(::std::vec::Vec::new()))
}

fn reset_state() {
    FREE_RESPONSE_CALLS.store(0, ::std::sync::atomic::Ordering::SeqCst);
    LAST_OPERATION.store(OP_NONE, ::std::sync::atomic::Ordering::SeqCst);
    response_records()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

fn make_header(key: &str, value: &str) -> ksbh_core::modules::abi::ModuleKvSlice {
    ksbh_core::modules::abi::ModuleKvSlice {
        key: bytes::Bytes::copy_from_slice(key.as_bytes()),
        value: bytes::Bytes::copy_from_slice(value.as_bytes()),
    }
}

fn alloc_response(
    status: http::StatusCode,
    headers: ::std::vec::Vec<ksbh_core::modules::abi::ModuleKvSlice>,
    body: bytes::Bytes,
) -> *const ksbh_core::modules::abi::ModuleResponse {
    let headers_ptr = headers.as_ptr();
    let headers_len = headers.len();

    let record = ::std::boxed::Box::new(ResponseRecord {
        response: ksbh_core::modules::abi::ModuleResponse {
            status_code: status.as_u16(),
            headers_ptr,
            headers_len,
            body,
        },
        headers,
    });

    let response_ptr = &record.response as *const ksbh_core::modules::abi::ModuleResponse;
    response_records()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(record);
    response_ptr
}

fn build_normal_response(
    ctx: &ksbh_modules_sdk::RequestContext<'_>,
) -> *const ksbh_core::modules::abi::ModuleResponse {
    let previous_seen = ctx
        .session
        .get("seen")
        .map(|bytes| ::std::string::String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default();

    let request_body = ::std::string::String::from_utf8_lossy(ctx.body).to_string();
    let current_value = format!("{}|{}", ctx.request.path, request_body);

    let session_set = ctx.session.set("seen", current_value.as_bytes());
    let session_set_ttl = ctx
        .session
        .set_with_ttl("seen_ttl", request_body.as_bytes(), 60);

    if !session_set || !session_set_ttl {
        return alloc_response(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            ::std::vec![make_header("content-type", "text/plain")],
            bytes::Bytes::from_static(b"failed to persist session state"),
        );
    }

    let score_before = ctx.metrics.get_score(ctx.metrics_key);
    let good_boy = ctx.metrics.good_boy(ctx.metrics_key);
    let score_after = ctx.metrics.get_score(ctx.metrics_key);

    let response_body = format!(
        "path={};method={};body={};seen_before={};score_before={};score_after={};good_boy={}",
        ctx.request.path,
        ctx.request.method,
        request_body,
        previous_seen,
        score_before,
        score_after,
        good_boy
    );

    alloc_response(
        http::StatusCode::OK,
        ::std::vec![
            make_header("content-type", "text/plain"),
            make_header("x-module-name", ctx.mod_name.as_str()),
            make_header("x-seen-before", &previous_seen),
            make_header("x-score-before", &score_before.to_string()),
            make_header("x-score-after", &score_after.to_string()),
            make_header("x-good-boy", &good_boy.to_string()),
            make_header("x-internal-path", ctx.internal_path),
        ],
        bytes::Bytes::from(response_body),
    )
}

fn build_cookie_response(kind: &str) -> *const ksbh_core::modules::abi::ModuleResponse {
    match kind {
        "present" => alloc_response(
            http::StatusCode::OK,
            ::std::vec![
                make_header("content-type", "text/plain"),
                make_header("set-cookie", "ksbh=already-present; Path=/; HttpOnly"),
            ],
            bytes::Bytes::from_static(b"cookie-present"),
        ),
        "malformed" => alloc_response(
            http::StatusCode::OK,
            ::std::vec![
                make_header("content-type", "text/plain"),
                make_header("set-cookie", "not-a-cookie"),
            ],
            bytes::Bytes::from_static(b"cookie-malformed"),
        ),
        _ => alloc_response(
            http::StatusCode::OK,
            ::std::vec![make_header("content-type", "text/plain")],
            bytes::Bytes::from_static(b"cookie-missing"),
        ),
    }
}

fn build_length_response(kind: &str) -> *const ksbh_core::modules::abi::ModuleResponse {
    let body = bytes::Bytes::from_static(b"length-body");
    match kind {
        "present" => alloc_response(
            http::StatusCode::OK,
            ::std::vec![
                make_header("content-type", "text/plain"),
                make_header("content-length", &body.len().to_string()),
            ],
            body,
        ),
        _ => alloc_response(
            http::StatusCode::OK,
            ::std::vec![make_header("content-type", "text/plain")],
            body,
        ),
    }
}

fn build_invalid_header_response() -> *const ksbh_core::modules::abi::ModuleResponse {
    let headers = ::std::vec![
        ksbh_core::modules::abi::ModuleKvSlice {
            key: bytes::Bytes::from_static(b"x-valid"),
            value: bytes::Bytes::from_static(b"ok"),
        },
        ksbh_core::modules::abi::ModuleKvSlice {
            key: bytes::Bytes::from_static(b"bad header name"),
            value: bytes::Bytes::from_static(b"still-valid-bytes"),
        },
        ksbh_core::modules::abi::ModuleKvSlice {
            key: bytes::Bytes::from_static(b"x-invalid-value"),
            value: bytes::Bytes::from_static(b"bad\r\nvalue"),
        },
    ];

    alloc_response(
        http::StatusCode::OK,
        headers,
        bytes::Bytes::from_static(b"invalid-header-response"),
    )
}

fn build_free_count_response() -> *const ksbh_core::modules::abi::ModuleResponse {
    let count = FREE_RESPONSE_CALLS.load(::std::sync::atomic::Ordering::SeqCst);
    LAST_OPERATION.store(OP_FREE_COUNT, ::std::sync::atomic::Ordering::SeqCst);

    alloc_response(
        http::StatusCode::OK,
        ::std::vec![make_header("x-free-response-calls", &count.to_string())],
        bytes::Bytes::from(count.to_string()),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn get_module_type() -> ksbh_core::modules::abi::ModuleType {
    ksbh_core::modules::abi::ModuleType {
        code: ksbh_core::modules::abi::ModuleTypeCode::Custom,
        custom_ptr: "dynamic-ffi-smoke".as_ptr(),
        custom_len: "dynamic-ffi-smoke".len(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn request_filter(
    ctx: *const ksbh_core::modules::abi::ModuleContext<'_>,
) -> *const ksbh_core::modules::abi::ModuleResponse {
    if ctx.is_null() {
        return ::std::ptr::null();
    }

    let ctx = ksbh_modules_sdk::ffi::convert_context(unsafe { &*ctx });
    if ctx.body == b"__count__" {
        return build_free_count_response();
    }

    if ctx.body == b"__reset__" {
        reset_state();
        return ::std::ptr::null();
    }

    match ctx.request.path.as_str() {
        "/ffi-smoke" => build_normal_response(&ctx),
        "/ffi-cookie" => build_cookie_response("missing"),
        "/ffi-cookie-present" => build_cookie_response("present"),
        "/ffi-cookie-malformed" => build_cookie_response("malformed"),
        "/ffi-length-missing" => build_length_response("missing"),
        "/ffi-length-present" => build_length_response("present"),
        "/ffi-invalid-header" => build_invalid_header_response(),
        "/pass" => ::std::ptr::null(),
        _ => build_normal_response(&ctx),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_response(
    headers_ptr: *const ksbh_core::modules::abi::ModuleKvSlice,
    headers_len: usize,
    body_ptr: *const u8,
    body_len: usize,
) {
    let operation = LAST_OPERATION.swap(OP_NONE, ::std::sync::atomic::Ordering::SeqCst);
    if operation != OP_FREE_COUNT {
        FREE_RESPONSE_CALLS.fetch_add(1, ::std::sync::atomic::Ordering::SeqCst);
    }

    if headers_ptr.is_null() && body_ptr.is_null() {
        return;
    }

    let mut records = response_records()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Some(position) = records.iter().position(|record| {
        record.response.headers_ptr == headers_ptr
            && record.response.headers_len == headers_len
            && record.response.body.as_ptr() == body_ptr
            && record.response.body.len() == body_len
    }) {
        let _ = records.remove(position);
    }
}
