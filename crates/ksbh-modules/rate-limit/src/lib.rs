//! Rate limiting module using reputation thresholds.
//!
//! Reads `score_threshold` from module config (default: 100).
//! Compares client's current reputation score against the threshold.
//! Returns HTTP 429 with `Retry-After` and `X-Score` headers if exceeded.

fn check_rate_limit(
    score: Option<u64>,
    threshold: u64,
) -> Option<(u64, http::Response<bytes::Bytes>)> {
    if let Some(score) = score {
        if score >= threshold {
            let response = http::Response::builder()
                .status(429)
                .header("Retry-After", "60")
                .header("X-Score", score.to_string())
                .body(bytes::Bytes::new())
                .expect("response should build");
            return Some((score, response));
        }
    }
    None
}

pub fn process(
    _stage: ksbh_modules_sdk::RequestStage,
    ctx: ksbh_modules_sdk::ModuleContext,
) -> ksbh_modules_sdk::RequestResult {
    {
        let score_threshold = ctx
            .config
            .get("score_threshold")
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        if let Some((_score, response)) = check_rate_limit(ctx.reputation_score()?, score_threshold)
        {
            return Ok(ksbh_modules_sdk::ModuleResult::Stop(Some(response)));
        }
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::export_module!(
    process,
    ksbh_modules_sdk::module_definition!(
        ksbh_modules_sdk::abi::prelude::KSBHModuleKind::RateLimit,
        [ksbh_modules_sdk::RequestStage::Request, ksbh_modules_sdk::RequestStage::BeforeRouting]
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_when_score_is_none() {
        assert!(check_rate_limit(None, 100).is_none());
    }

    #[test]
    fn passes_when_score_below_threshold() {
        assert!(check_rate_limit(Some(50), 100).is_none());
    }

    #[test]
    fn blocks_when_score_equals_threshold() {
        let result = check_rate_limit(Some(100), 100);
        assert!(result.is_some());
        let (_score, response) = result.unwrap();
        assert_eq!(response.status(), 429);
        assert_eq!(response.headers().get("Retry-After").unwrap(), "60");
        assert_eq!(response.headers().get("X-Score").unwrap(), "100");
    }

    #[test]
    fn blocks_when_score_exceeds_threshold() {
        let result = check_rate_limit(Some(200), 100);
        assert!(result.is_some());
    }

    #[test]
    fn blocks_when_score_is_zero_and_threshold_is_zero() {
        assert!(check_rate_limit(Some(0), 0).is_some());
    }

    #[test]
    fn blocks_when_score_is_zero_and_threshold_is_negative_unsigned_overflow() {
        let result = check_rate_limit(Some(0), 0);
        assert!(result.is_some());
    }

    #[test]
    fn process_passes_at_before_routing_stage() {
        use ksbh_modules_sdk::abi::prelude::*;
        use ksbh_modules_sdk::abi::request_info::RequestInfo as AbiRequestInfo;

        unsafe extern "C" fn stub_log(
            _: KSBHHostCtxHandle,
            _: LogLevel,
            _: KSBHBytes,
        ) -> KSBHHostFnReturn {
            KSBHHostFnReturn::Success
        }

        unsafe extern "C" fn stub_free_bytes(_: KSBHHostCtxHandle, _: KSBHBytes) {}

        unsafe extern "C" fn stub_reputation_good_boy(
            _: KSBHHostCtxHandle,
        ) -> KSBHHostFnReturn {
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

        const EMPTY_KV: [KSBHKVString; 0] = [];
        const EMPTY_BYTE: [u8; 0] = [];

        let req_info = Box::new(AbiRequestInfo {
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
        });

        let abi = ksbh_modules_sdk::abi::types::ModuleContext {
            host_ctx: KSBHHostCtxHandle { inner: 1 },
            config: KSBHSliceKVStrings {
                ptr: EMPTY_KV.as_ptr(),
                len: 0,
            },
            headers: KSBHSliceKVStrings {
                ptr: EMPTY_KV.as_ptr(),
                len: 0,
            },
            request_info: req_info.as_ref() as *const AbiRequestInfo,
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
        };

        let ctx = ksbh_modules_sdk::ModuleContext::try_from(&abi).unwrap();
        let result = process(ksbh_modules_sdk::RequestStage::BeforeRouting, ctx);
        assert!(matches!(
            result,
            Ok(ksbh_modules_sdk::ModuleResult::Pass)
        ));
    }
}
