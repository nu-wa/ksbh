use ksbh_modules_abi::prelude::*;
use ksbh_modules_abi::request_info::RequestInfo as AbiRequestInfo;
use ksbh_modules_abi::types::ModuleContext as AbiModuleContext;
use ksbh_modules_sdk::{ModuleError, ModuleResult};

// ── Stub host function pointers ──────────────────────────────────────

unsafe extern "C" fn stub_log(
    _: KSBHHostCtxHandle,
    _: LogLevel,
    _: KSBHBytes,
) -> KSBHHostFnReturn {
    KSBHHostFnReturn::Success
}

unsafe extern "C" fn stub_free_bytes(_: KSBHHostCtxHandle, _: KSBHBytes) {}

unsafe extern "C" fn stub_reputation_good_boy(_: KSBHHostCtxHandle) -> KSBHHostFnReturn {
    KSBHHostFnReturn::Success
}

unsafe extern "C" fn stub_reputation_get_score(
    _: KSBHHostCtxHandle,
    out: *mut u64,
) -> KSBHHostFnReturn {
    unsafe {
        *out = 42;
    }
    KSBHHostFnReturn::Success
}

unsafe extern "C" fn stub_signal_get(
    _: KSBHHostCtxHandle,
    _: KSBHHostSignalKind,
    out: *mut u64,
) -> KSBHHostFnReturn {
    unsafe {
        *out = 100;
    }
    KSBHHostFnReturn::Success
}

unsafe extern "C" fn stub_session_get(
    _: KSBHHostCtxHandle,
    _: KSBHBytes,
    _out: *mut KSBHBytes,
) -> KSBHHostFnReturn {
    KSBHHostFnReturn::NotFound
}

unsafe extern "C" fn stub_session_set(
    _: KSBHHostCtxHandle,
    _: KSBHBytes,
    _: KSBHBytes,
    _: u64,
) -> KSBHHostFnReturn {
    KSBHHostFnReturn::Success
}

unsafe extern "C" fn stub_session_shared_get(
    _: KSBHHostCtxHandle,
    _: KSBHBytes,
    _out: *mut KSBHBytes,
) -> KSBHHostFnReturn {
    KSBHHostFnReturn::NotFound
}

unsafe extern "C" fn stub_session_shared_set(
    _: KSBHHostCtxHandle,
    _: KSBHBytes,
    _: KSBHBytes,
    _: u64,
) -> KSBHHostFnReturn {
    KSBHHostFnReturn::Success
}

// ── Empty const slices for zero-length ABI fields ──────────────────

const EMPTY_KV: [KSBHKVString; 0] = [];
const EMPTY_BYTE: [u8; 0] = [];

// ── Helper: build a minimal valid ABI ModuleContext ──────────────────

fn make_req_info() -> Box<AbiRequestInfo> {
    Box::new(AbiRequestInfo {
        uri: KSBHString::from_str("/test"),
        host: KSBHString::from_str("example.com"),
        method: KSBHString::from_str("GET"),
        path: KSBHString::from_str("/test"),
        scheme: KSBHString::from_str("https"),
        port: 443,
        is_ws_handshake: 0,
        query_params: KSBHSliceKVStrings {
            ptr: EMPTY_KV.as_ptr(),
            len: 0,
        },
    })
}

fn build_minimal_abi_ctx(req_info: &AbiRequestInfo) -> AbiModuleContext {
    AbiModuleContext {
        host_ctx: KSBHHostCtxHandle { inner: 1 },
        config: KSBHSliceKVStrings {
            ptr: EMPTY_KV.as_ptr(),
            len: 0,
        },
        headers: KSBHSliceKVStrings {
            ptr: EMPTY_KV.as_ptr(),
            len: 0,
        },
        request_info: req_info as *const AbiRequestInfo,
        body: KSBHBytes {
            ptr: EMPTY_BYTE.as_ptr(),
            len: 0,
        },
        cookie_header: KSBHString::from_str(""),
        reputation_key: KSBHBytes::from_slice(&[]),
        internal_path: KSBHString::from_str("/_internal"),
        session_id: SessionID {
            inner: [0u8; 16],
        },
        h_log_fn: stub_log,
        h_free_bytes: stub_free_bytes,
        h_reputation_good_boy_fn: stub_reputation_good_boy,
        h_reputation_get_score_fn: stub_reputation_get_score,
        h_signal_get_fn: stub_signal_get,
        h_session_get_fn: stub_session_get,
        h_session_set_fn: stub_session_set,
        h_session_shared_get_fn: stub_session_shared_get,
        h_session_shared_set_fn: stub_session_shared_set,
    }
}

// ── Tests: ModuleContext::try_from ───────────────────────────────────

#[test]
fn valid_ctx_constructs() {
    let req_info = make_req_info();
    let abi = build_minimal_abi_ctx(&req_info);
    let result = ksbh_modules_sdk::ModuleContext::try_from(&abi);
    assert!(
        result.is_ok(),
        "try_from should succeed with valid input: {:?}",
        result.err()
    );
}

#[test]
fn null_request_info_fails() {
    let _req_info = make_req_info();
    let mut abi = build_minimal_abi_ctx(&_req_info);
    abi.request_info = std::ptr::null();
    let result = ksbh_modules_sdk::ModuleContext::try_from(&abi);
    assert!(result.is_err(), "null request_info should fail");
}

#[test]
fn empty_config_constructs() {
    let req_info = make_req_info();
    let abi = build_minimal_abi_ctx(&req_info);
    let result = ksbh_modules_sdk::ModuleContext::try_from(&abi);
    assert!(result.is_ok(), "empty config should succeed");
}

#[test]
fn valid_config_kv_constructs() {
    let kvs = vec![
        KSBHKVString {
            key: KSBHString::from_str("issuer_url"),
            value: KSBHString::from_str("https://example.com"),
        },
        KSBHKVString {
            key: KSBHString::from_str("client_id"),
            value: KSBHString::from_str("my-client"),
        },
    ];

    let req_info = make_req_info();
    let mut abi = build_minimal_abi_ctx(&req_info);
    abi.config = KSBHSliceKVStrings {
        ptr: kvs.as_ptr(),
        len: kvs.len(),
    };

    let result = ksbh_modules_sdk::ModuleContext::try_from(&abi);
    assert!(
        result.is_ok(),
        "valid config kv pairs should succeed: {:?}",
        result.err()
    );
}

// ── Unit tests: ModuleError constructors ──────────────────────────────

#[test]
fn module_error_response_variants() {
    let err = ModuleError::bad_request("bad input");
    assert!(matches!(err, ModuleError::Response { .. }));

    let err = ModuleError::unauthorized("not allowed");
    assert!(matches!(err, ModuleError::Response { .. }));

    let err = ModuleError::forbidden("nope");
    assert!(matches!(err, ModuleError::Response { .. }));

    let err = ModuleError::not_found("missing");
    assert!(matches!(err, ModuleError::Response { .. }));

    let err = ModuleError::too_many_requests("rate limited");
    assert!(matches!(err, ModuleError::Response { .. }));

    let err = ModuleError::internal_error("boom");
    assert!(matches!(err, ModuleError::Response { .. }));
}

#[test]
fn module_error_host_abi_critical_variants() {
    let err = ModuleError::host("host blew up");
    assert!(matches!(err, ModuleError::Host { .. }));

    let err = ModuleError::abi("bad ABI data");
    assert!(matches!(err, ModuleError::Abi { .. }));

    let err = ModuleError::critical(anyhow::anyhow!("critical failure"));
    assert!(matches!(err, ModuleError::Critical { .. }));

    let err = ModuleError::missing_config("issuer_url");
    assert!(format!("{:?}", err).contains("issuer_url"));
}

#[test]
fn module_error_display_and_debug() {
    let err = ModuleError::bad_request("test message");
    let display = format!("{}", err);
    assert!(display.contains("test message"));

    let debug = format!("{:?}", err);
    assert!(debug.contains("test message"));
}

#[test]
fn module_result_variants() {
    let pass = ModuleResult::Pass;
    assert!(matches!(pass, ModuleResult::Pass));

    let stop_none = ModuleResult::Stop(None);
    assert!(matches!(stop_none, ModuleResult::Stop(None)));

    let stop_some = ModuleResult::Stop(Some(
        http::Response::builder()
            .status(http::StatusCode::OK)
            .body(bytes::Bytes::new())
            .unwrap(),
    ));
    assert!(matches!(stop_some, ModuleResult::Stop(Some(_))));

    let err = ModuleResult::Error(Some(
        http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from("bad"))
            .unwrap(),
    ));
    assert!(matches!(err, ModuleResult::Error(Some(_))));

    let err_none = ModuleResult::Error(None);
    assert!(matches!(err_none, ModuleResult::Error(None)));
}

#[test]
#[allow(invalid_from_utf8)]
fn module_error_from_traits() {
    // anyhow::Error → Critical
    let e: anyhow::Error = anyhow::anyhow!("test anyhow");
    let err: ModuleError = e.into();
    assert!(matches!(err, ModuleError::Critical { .. }));

    // String → Critical
    let err: ModuleError = String::from("test string").into();
    assert!(matches!(err, ModuleError::Critical { .. }));

    // &str → Critical
    let err: ModuleError = "test str".into();
    assert!(matches!(err, ModuleError::Critical { .. }));

    // io::Error → Critical
    let err: ModuleError = std::io::Error::new(std::io::ErrorKind::Other, "io").into();
    assert!(matches!(err, ModuleError::Critical { .. }));

    // Utf8Error → Abi
    let invalid: &[u8] = &[0xFF, 0xFE];
    let utf8_err = std::str::from_utf8(invalid).unwrap_err();
    let err: ModuleError = utf8_err.into();
    assert!(matches!(err, ModuleError::Abi { .. }));
}
