//! Module chain execution.
//!
//! The chain runner walks a slice of [`RequestMatchModule`]s, dispatching each
//! one to the configured [`ModuleDispatcher`] and translating the per-module
//! outcomes ([`ModuleCallOutcome`]) into a unified [`ChainOutcome`].
//!
//! Two callers (post-routing request filter and global early request filter)
//! used to duplicate this loop; the runner lives here so the FFI dispatch,
//! the cookie-injection step, the error mapping, and the metric push are
//! defined exactly once.

use ksbh_modules_abi::prelude::KSBHModuleStage;
use ksbh_types::prelude::ProxyProviderError;

use crate::modules::runtime::{
    module_host::{ModuleHost, ModuleHostError},
    ModuleCallInput, ModuleCallOutcome,
};
use crate::{
    cookies::{CookieSettings, ProxyCookie},
    metrics::module_metric::ModuleMetric,
    proxy::ObservedRequest,
    routing::request_match::RequestMatchModule,
};

/// Result of running a module chain.
///
/// The proxy maps this to a [`ksbh_types::prelude::ProxyDecision`] at the call
/// site; the runner itself stays decoupled from pingora-specific concerns.
#[derive(Debug)]
pub enum ChainOutcome {
    /// Every module returned [`ModuleCallOutcome::Pass`]; continue normal
    /// processing.
    Continue,
    /// A module produced a response. Cookie injection has already been applied
    /// if `needs_session_cookie` was set and the response did not already set
    /// the proxy cookie.
    Reply(http::Response<bytes::Bytes>),
    /// A module referenced in the routing config is not loaded.
    /// The chain skips this module and continues to the next.
    ModuleNotFound {
        module_name: String,
    },
    /// A module failed (either via the dispatcher's [`Err`] arm or by
    /// returning [`ModuleCallOutcome::Error`]). The body is ready to be sent
    /// with HTTP 500.
    ServerError(bytes::Bytes),
}

/// Abstracts the module dispatch seam so the chain runner can be unit-tested
/// without loading a real cdylib module.
pub trait ModuleDispatcher {
    /// Dispatch a single module invocation. Returns the same value as
    /// [`ModuleHost::call_module`].
    fn dispatch(
        &self,
        input: ModuleCallInput<'_>,
    ) -> Result<ModuleCallOutcome, ModuleHostError>;
}

impl ModuleDispatcher for ModuleHost {
    fn dispatch(
        &self,
        input: ModuleCallInput<'_>,
    ) -> Result<ModuleCallOutcome, ModuleHostError> {
        ModuleHost::call_module(self, input)
    }
}

/// Run a chain of modules against a single request.
///
/// The caller is responsible for translating the returned [`ChainOutcome`] to
/// a [`ksbh_types::prelude::ProxyDecision`] and for any pingora-specific
/// I/O such as `session.write_response`.
///
/// `metric_factory` is invoked once per module (and once again with
/// `module_replied=true` if a module terminates the chain); it is the
/// caller's hook for choosing the right constructor for the stage in flight
/// (e.g. `ModuleMetric::new_request` vs `ModuleMetric::new_early`).
#[allow(clippy::too_many_arguments)]
pub fn run_chain<D: ModuleDispatcher + ?Sized>(
    dispatcher: &D,
    modules: &[RequestMatchModule],
    stage: KSBHModuleStage,
    observed: &ObservedRequest,
    headers: &http::HeaderMap,
    body: Option<&bytes::Bytes>,
    internal_path: &str,
    needs_session_cookie: bool,
    cookie_settings: &CookieSettings,
    metric_factory: impl Fn(&str, f64, bool, bool) -> ModuleMetric,
    metrics_out: &mut Vec<ModuleMetric>,
) -> Result<ChainOutcome, ProxyProviderError> {
    for module in modules {
        let start = ::std::time::Instant::now();

        let mod_call_result = dispatcher.dispatch(ModuleCallInput {
            stage,
            module_name: module.name.as_str(),
            module_type: module.mod_spec.r#type.clone(),
            config: &module.config_values,
            observed,
            headers,
            body,
            internal_path,
            needs_session_cookie,
        });

        let mod_exec_time = start.elapsed().as_secs_f64();

        match mod_call_result {
            Err(error) => {
                match &error {
                    ModuleHostError::ModuleNotFound => {
                        tracing::warn!(
                            "Module {} not loaded (missing .so?), skipping in chain",
                            module.name
                        );
                        metrics_out.push(metric_factory(
                            module.name.as_str(),
                            mod_exec_time,
                            true,
                            false,
                        ));
                        // Skip this module, continue to the next in the chain
                        continue;
                    }
                    _ => {
                        tracing::error!("Module {} error: {:?}", module.name, error);
                        metrics_out.push(metric_factory(
                            module.name.as_str(),
                            mod_exec_time,
                            true,
                            true,
                        ));
                        return Ok(ChainOutcome::ServerError(bytes::Bytes::from(format!(
                            "module {} failed",
                            module.name
                        ))));
                    }
                }
            }
            Ok(ModuleCallOutcome::Pass) => {
                tracing::debug!("Module {} executed successfully", module.name);
            }
            Ok(ModuleCallOutcome::Stop(mut response)) => {
                tracing::debug!(
                    "Module {} wrote a response, stopping module chain",
                    module.name
                );

                if needs_session_cookie
                    && !response_sets_proxy_cookie(&response, &cookie_settings.name)
                {
                    let cookie = ProxyCookie::new(
                        observed.http_request.host.as_str(),
                        observed.session_id,
                    );

                    let cookie_header = cookie.to_cookie_header(cookie_settings).map_err(|e| {
                        tracing::error!(
                            "Failed to create proxy cookie for module {}: {}",
                            module.name,
                            e
                        );
                        ProxyProviderError::InternalErrorDetailed(e.to_string())
                    })?;

                    let cookie_header = http::HeaderValue::from_str(&cookie_header)
                        .map_err(ProxyProviderError::from)?;
                    response
                        .headers_mut()
                        .append(http::header::SET_COOKIE, cookie_header);
                }

                metrics_out.push(metric_factory(
                    module.name.as_str(),
                    mod_exec_time,
                    true,
                    true,
                ));
                return Ok(ChainOutcome::Reply(response));
            }
            Ok(ModuleCallOutcome::Error(message)) => {
                tracing::error!("Module {} returned error: {}", module.name, message);
                metrics_out.push(metric_factory(
                    module.name.as_str(),
                    mod_exec_time,
                    true,
                    true,
                ));
                return Ok(ChainOutcome::ServerError(bytes::Bytes::from(message)));
            }
        }

        // Pass arm: record the metric with module_replied=false at the end of
        // the iteration.
        metrics_out.push(metric_factory(
            module.name.as_str(),
            mod_exec_time,
            true,
            false,
        ));
    }

    Ok(ChainOutcome::Continue)
}

impl ModuleHost {
    /// Thin wrapper around [`run_chain`] that uses `self` as the dispatcher.
    /// See the free function for full semantics.
    #[allow(clippy::too_many_arguments)]
    pub fn run_chain(
        &self,
        modules: &[RequestMatchModule],
        stage: KSBHModuleStage,
        observed: &ObservedRequest,
        headers: &http::HeaderMap,
        body: Option<&bytes::Bytes>,
        internal_path: &str,
        needs_session_cookie: bool,
        cookie_settings: &CookieSettings,
        metric_factory: impl Fn(&str, f64, bool, bool) -> ModuleMetric,
        metrics_out: &mut Vec<ModuleMetric>,
    ) -> Result<ChainOutcome, ProxyProviderError> {
        run_chain(
            self,
            modules,
            stage,
            observed,
            headers,
            body,
            internal_path,
            needs_session_cookie,
            cookie_settings,
            metric_factory,
            metrics_out,
        )
    }
}

/// Returns `true` if any `Set-Cookie` header on `response` already sets a
/// cookie named `cookie_name`. Used to avoid appending the proxy cookie twice
/// when a module has set it itself.
fn response_sets_proxy_cookie(response: &http::Response<bytes::Bytes>, cookie_name: &str) -> bool {
    for header in response.headers().get_all(http::header::SET_COOKIE) {
        let Ok(header_value) = header.to_str() else {
            continue;
        };

        let Some(first_segment) = header_value.split(';').next() else {
            continue;
        };

        let Some((parsed_cookie_name, _)) = first_segment.split_once('=') else {
            continue;
        };

        if parsed_cookie_name.trim() == cookie_name {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::std::sync::atomic::{AtomicUsize, Ordering};
    use ::std::sync::{Arc, Mutex};

    use ksbh_types::KsbhStr;
    use ksbh_types::requests::http_request::HttpRequest;

    use crate::modules::ModuleConfigurationSpec;

    /// A test dispatcher that returns a queued sequence of outcomes and
    /// records how many times it was invoked.
    struct TestDispatcher {
        outcomes: Mutex<Vec<Result<ModuleCallOutcome, ModuleHostError>>>,
        call_count: AtomicUsize,
    }

    impl TestDispatcher {
        fn new(outcomes: Vec<Result<ModuleCallOutcome, ModuleHostError>>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes),
                call_count: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl ModuleDispatcher for TestDispatcher {
        fn dispatch(
            &self,
            _input: ModuleCallInput<'_>,
        ) -> Result<ModuleCallOutcome, ModuleHostError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let mut guard = self.outcomes.lock().unwrap_or_else(|p| p.into_inner());
            if guard.is_empty() {
                panic!("TestDispatcher ran out of queued outcomes");
            }
            guard.remove(0)
        }
    }

    fn make_module(name: &str) -> RequestMatchModule {
        RequestMatchModule {
            name: Arc::new(KsbhStr::new(name)),
            mod_spec: Arc::new(ModuleConfigurationSpec {
                name: name.to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom(name.to_string()),
                weight: 0,
                global: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            }),
            config_values: Arc::new(hashbrown::HashMap::new()),
            config_kv_slice: Arc::new(Vec::new()),
        }
    }

    fn make_observed(host: &str) -> ObservedRequest {
        ObservedRequest {
            req_id: uuid::Uuid::new_v4(),
            started_at: ::std::time::Instant::now(),
            client: crate::proxy::ClientInformation {
                ip: ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(127, 0, 0, 1)),
                header_hash: [0u8; 32],
                reputation_key: [0u8; 32],
            },
            session_id: uuid::Uuid::from_u128(0xDEAD_BEEF_u128),
            http_request: HttpRequest::t_create(host, Some(b"/"), Some("GET")),
            is_websocket_handshake: false,
        }
    }

    fn make_cookie_settings() -> CookieSettings {
        // 64 bytes of zero is the minimum length accepted by cookie::Key.
        let key_bytes = [7u8; 64];
        CookieSettings {
            key: cookie::Key::try_from(&key_bytes[..])
                .expect("64-byte key is a valid cookie key"),
            name: "ksbh_session".to_string(),
            secure: true,
        }
    }

    fn stop_response(status: u16, set_cookies: &[&str]) -> http::Response<bytes::Bytes> {
        let mut builder = http::Response::builder().status(status);
        for value in set_cookies {
            builder = builder.header(http::header::SET_COOKIE, *value);
        }
        builder
            .body(bytes::Bytes::from_static(b""))
            .expect("response should build")
    }

    fn metric_factory(name: &str, exec_time: f64, _global: bool, module_replied: bool) -> ModuleMetric {
        ModuleMetric::new_request(name, exec_time, true, module_replied)
    }

    #[test]
    fn stop_short_circuits_the_chain() {
        let modules = vec![make_module("a"), make_module("b")];
        let dispatcher = TestDispatcher::new(vec![
            Ok(ModuleCallOutcome::Stop(stop_response(200, &[]))),
            Ok(ModuleCallOutcome::Stop(stop_response(200, &[]))),
        ]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::Request,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            false,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err");

        match outcome {
            ChainOutcome::Reply(response) => {
                assert_eq!(response.status(), 200);
            }
            other => panic!("expected ChainOutcome::Reply, got {other:?}"),
        }
        assert_eq!(dispatcher.calls(), 1, "module B must not be called");
        assert_eq!(metrics.len(), 1, "only the stopping module is recorded");
        assert!(metrics[0].module_replied());
    }

    #[test]
    fn stop_without_cookie_appends_proxy_cookie_when_needed() {
        let modules = vec![make_module("a")];
        let dispatcher = TestDispatcher::new(vec![Ok(ModuleCallOutcome::Stop(stop_response(
            200,
            &[],
        )))]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::Request,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            true,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err");

        match outcome {
            ChainOutcome::Reply(response) => {
                let set_cookies: Vec<_> = response
                    .headers()
                    .get_all(http::header::SET_COOKIE)
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .collect();
                assert_eq!(set_cookies.len(), 1, "one cookie should be appended");
                let prefix = format!("{}=", cookie_settings.name);
                assert!(
                    set_cookies[0].starts_with(&prefix),
                    "appended cookie should be the proxy cookie, got {}",
                    set_cookies[0]
                );
            }
            other => panic!("expected ChainOutcome::Reply, got {other:?}"),
        }
    }

    #[test]
    fn stop_with_existing_proxy_cookie_is_not_duplicated() {
        // A module that already set the proxy cookie must not see it
        // re-appended; otherwise downstream callers would see two Set-Cookie
        // headers for the same name.
        let existing = "ksbh_session=existing; Path=/; HttpOnly";
        let modules = vec![make_module("a")];
        let dispatcher = TestDispatcher::new(vec![Ok(ModuleCallOutcome::Stop(stop_response(
            200,
            &[existing],
        )))]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::Request,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            true,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err");

        match outcome {
            ChainOutcome::Reply(response) => {
                let proxy_cookie_count = response
                    .headers()
                    .get_all(http::header::SET_COOKIE)
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .filter(|v| v.split(';').next().and_then(|s| s.split_once('='))
                        .map(|(name, _)| name.trim() == cookie_settings.name)
                        .unwrap_or(false))
                    .count();
                assert_eq!(
                    proxy_cookie_count, 1,
                    "proxy cookie must not be duplicated"
                );
            }
            other => panic!("expected ChainOutcome::Reply, got {other:?}"),
        }
    }

    #[test]
    fn module_not_found_is_skipped_in_chain() {
        let modules = vec![make_module("a"), make_module("b")];
        // Module A is NotFound, Module B passes normally
        let dispatcher = TestDispatcher::new(vec![
            Err(ModuleHostError::ModuleNotFound),
            Ok(ModuleCallOutcome::Pass),
        ]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::Request,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            false,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err");

        assert!(matches!(outcome, ChainOutcome::Continue),
            "Chain should continue when ModuleNotFound is skipped");
        assert_eq!(dispatcher.calls(), 2, "both modules should be attempted");
        assert_eq!(metrics.len(), 2, "both modules should record metrics");
        assert!(!metrics[0].module_replied(), "skipped module should not mark module_replied");
        assert!(!metrics[1].module_replied(), "passing module should not mark module_replied");
    }

    #[test]
    fn dispatch_internal_error_returns_server_error() {
        let modules = vec![make_module("a")];
        let dispatcher = TestDispatcher::new(vec![Err(ModuleHostError::InternalError(
            "redis down".to_string(),
        ))]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::Request,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            false,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err");

        match outcome {
            ChainOutcome::ServerError(body) => {
                let body_str = ::std::str::from_utf8(&body).expect("body is utf-8");
                assert_eq!(body_str, "module a failed");
            }
            other => panic!("expected ChainOutcome::ServerError for internal error, got {other:?}"),
        }
    }

    #[test]
    fn module_error_outcome_returns_server_error_with_message_body() {
        let modules = vec![make_module("a")];
        let dispatcher = TestDispatcher::new(vec![Ok(ModuleCallOutcome::Error(
            "boom".to_string(),
        ))]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::BeforeRouting,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            false,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err for a module-emitted error");

        match outcome {
            ChainOutcome::ServerError(body) => {
                assert_eq!(body.as_ref(), b"boom");
            }
            other => panic!("expected ChainOutcome::ServerError, got {other:?}"),
        }
        assert_eq!(metrics.len(), 1);
        assert!(metrics[0].module_replied());
    }

    #[test]
    fn all_modules_pass_continues_and_records_per_module_metrics() {
        let modules = vec![make_module("a"), make_module("b"), make_module("c")];
        let dispatcher = TestDispatcher::new(vec![
            Ok(ModuleCallOutcome::Pass),
            Ok(ModuleCallOutcome::Pass),
            Ok(ModuleCallOutcome::Pass),
        ]);
        let observed = make_observed("example.com");
        let cookie_settings = make_cookie_settings();
        let mut metrics = Vec::new();

        let outcome = run_chain(
            &dispatcher,
            &modules,
            KSBHModuleStage::Request,
            &observed,
            &http::HeaderMap::new(),
            None,
            "/_ksbh/modules",
            false,
            &cookie_settings,
            metric_factory,
            &mut metrics,
        )
        .expect("run_chain should not return Err on a clean Pass chain");

        assert!(matches!(outcome, ChainOutcome::Continue));
        assert_eq!(dispatcher.calls(), 3);
        assert_eq!(metrics.len(), 3, "one metric per module");
        for metric in &metrics {
            assert!(!metric.module_replied(), "Pass must not mark module_replied");
        }
    }

    #[test]
    fn response_sets_proxy_cookie_detects_matching_cookie() {
        let response = stop_response(200, &["ksbh_session=abc; Path=/; HttpOnly"]);
        assert!(response_sets_proxy_cookie(&response, "ksbh_session"));
    }

    #[test]
    fn response_sets_proxy_cookie_ignores_unrelated_cookies() {
        let response = stop_response(200, &["other_cookie=abc; Path=/"]);
        assert!(!response_sets_proxy_cookie(&response, "ksbh_session"));
    }
}
