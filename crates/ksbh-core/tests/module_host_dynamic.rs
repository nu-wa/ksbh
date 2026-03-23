struct TestSession {
    request_header: pingora_http::RequestHeader,
    request_body: Option<bytes::Bytes>,
    response: Option<http::Response<bytes::Bytes>>,
    sent: bool,
    client_addr: Option<::std::net::IpAddr>,
}

impl TestSession {
    fn new(
        host: &str,
        path: &[u8],
        method: &str,
        request_body: Option<bytes::Bytes>,
    ) -> Result<Self, ksbh_types::prelude::ProxyProviderError> {
        let mut request_header = pingora_http::RequestHeader::build_no_case(method, path, None)
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        request_header
            .insert_header(http::header::HOST, host)
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        Ok(Self {
            request_header,
            request_body,
            response: None,
            sent: false,
            client_addr: Some(::std::net::IpAddr::V4(::std::net::Ipv4Addr::LOCALHOST)),
        })
    }
}

#[async_trait::async_trait]
impl ksbh_types::prelude::ProxyProviderSession for TestSession {
    fn headers(&self) -> http::request::Parts {
        self.request_header.as_owned_parts()
    }

    fn get_header(&self, header_name: http::HeaderName) -> Option<&http::header::HeaderValue> {
        self.request_header.headers.get(header_name)
    }

    fn set_request_uri(&mut self, uri: http::Uri) {
        self.request_header.set_uri(uri);
    }

    fn response_written(&self) -> Option<http::Response<bytes::Bytes>> {
        self.response.clone()
    }

    fn response_sent(&self) -> bool {
        self.sent
    }

    fn client_addr(&self) -> Option<::std::net::IpAddr> {
        self.client_addr
    }

    async fn write_response(
        &mut self,
        response: http::Response<bytes::Bytes>,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        self.response = Some(response);
        self.sent = true;
        Ok(())
    }

    async fn read_request_body(
        &mut self,
    ) -> Result<Option<bytes::Bytes>, ksbh_types::prelude::ProxyProviderError> {
        Ok(self.request_body.clone())
    }
}

fn dynamic_module_library_path() -> Result<::std::path::PathBuf, Box<dyn ::std::error::Error>> {
    if let Ok(path) = ::std::env::var("KSBH_DYNAMIC_MODULE_LIB") {
        return Ok(::std::path::PathBuf::from(path));
    }

    let extension = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };

    let workspace_root = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("failed to locate workspace root from ksbh-core manifest dir")?
        .to_path_buf();

    Ok(workspace_root
        .join("target")
        .join("debug")
        .join(format!("libdynamic_ffi_smoke.{}", extension)))
}

fn create_host()
-> Result<ksbh_core::modules::abi::module_host::ModuleHost, Box<dyn ::std::error::Error>> {
    let cookie_settings = ::std::sync::Arc::new(ksbh_core::cookies::CookieSettings {
        key: cookie::Key::try_from(
            b"0123456789012345678901234567890101234567890123456789012345678901".as_slice(),
        )
        .map_err(|_| "invalid static cookie key")?,
        name: "ksbh".to_string(),
        secure: false,
    });

    let store = ::std::sync::Arc::new(ksbh_core::storage::redis_hashmap::RedisHashMap::new(
        Some(tokio::time::Duration::from_secs(60)),
        Some(tokio::time::Duration::from_secs(60)),
        Some(::std::sync::Arc::new(ksbh_core::storage::Storage::empty())),
    ));

    Ok(ksbh_core::modules::abi::module_host::ModuleHost::new(
        cookie_settings,
        store,
    ))
}

fn shared_host()
-> Result<&'static ksbh_core::modules::abi::module_host::ModuleHost, Box<dyn ::std::error::Error>> {
    static SHARED_HOST: ::std::sync::OnceLock<ksbh_core::modules::abi::module_host::ModuleHost> =
        ::std::sync::OnceLock::new();

    if let Some(host) = SHARED_HOST.get() {
        return Ok(host);
    }

    let host = create_host()?;
    let dynamic_module_path = dynamic_module_library_path()?;
    host.load_module(&dynamic_module_path)?;

    let _ = SHARED_HOST.set(host);
    SHARED_HOST
        .get()
        .ok_or("failed to initialize shared module host".into())
}

fn seed_metrics_score(
    host: &ksbh_core::modules::abi::module_host::ModuleHost,
    session_id: uuid::Uuid,
    score: u64,
) -> Result<(), Box<dyn ::std::error::Error>> {
    let encoded = rmp_serde::to_vec(&ksbh_core::metrics::AtomicU64Wrapper::new(score))?;
    let metrics_key =
        ksbh_core::storage::module_session_key::ModuleSessionKey::user_session(session_id);
    let _ = host.session_store().set_sync(metrics_key, encoded);
    Ok(())
}

fn build_request_context(
    session: &mut TestSession,
    path: &[u8],
    body: Option<bytes::Bytes>,
    session_id: uuid::Uuid,
    needs_session_cookie: bool,
) -> ksbh_core::modules::abi::ModuleRequestContext<'static> {
    let http_request = ksbh_types::requests::http_request::HttpRequest::t_create(
        "example.test",
        Some(path),
        Some("POST"),
    );

    let metrics_key = Box::leak(Box::new(*session_id.as_bytes()));

    ksbh_core::modules::abi::ModuleRequestContext::new(
        session,
        &http_request,
        needs_session_cookie,
        *session_id.as_bytes(),
        metrics_key,
        body,
        "/_ksbh_internal",
    )
}

fn build_params() -> ksbh_core::modules::abi::ModuleCallParams {
    let config = ::std::sync::Arc::new(Vec::<ksbh_core::modules::abi::ModuleKvSlice>::new());
    let module_name = ::std::sync::Arc::new(ksbh_types::KsbhStr::new("dynamic-ffi-smoke"));

    ksbh_core::modules::abi::ModuleCallParams::new(
        ksbh_core::modules::ModuleConfigurationType::Custom("dynamic-ffi-smoke".to_string()),
        config,
        module_name,
    )
}

fn response_header_string(
    response: &http::Response<bytes::Bytes>,
    header_name: &str,
) -> Option<::std::string::String> {
    response
        .headers()
        .get(header_name)
        .and_then(|value| value.to_str().ok())
        .map(::std::string::String::from)
}

fn dynamic_smoke_loops() -> usize {
    ::std::env::var("KSBH_DYNAMIC_SMOKE_LOOPS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(25)
}

#[tokio::test]
async fn dynamic_module_host_loads_real_cdylib_and_persists_state()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;

    for _ in 0..dynamic_smoke_loops() {
        let session_id = uuid::Uuid::new_v4();
        seed_metrics_score(host, session_id, 120)?;

        let params = build_params();

        let mut first_session = TestSession::new(
            "example.test",
            b"/ffi-smoke",
            "POST",
            Some(bytes::Bytes::from_static(b"alpha")),
        )?;
        let first_req_ctx = build_request_context(
            &mut first_session,
            b"/ffi-smoke",
            Some(bytes::Bytes::from_static(b"alpha")),
            session_id,
            false,
        );

        host.call_module(&first_req_ctx, &params, &mut first_session)
            .await?;

        let first_response = first_session
            .response
            .clone()
            .ok_or("first module call did not write a response")?;

        assert_eq!(first_response.status(), http::StatusCode::OK);
        assert_eq!(
            response_header_string(&first_response, "x-seen-before").as_deref(),
            Some("")
        );
        assert_eq!(
            response_header_string(&first_response, "x-score-before").as_deref(),
            Some("120")
        );
        assert_eq!(
            response_header_string(&first_response, "x-score-after").as_deref(),
            Some("70")
        );
        assert_eq!(
            response_header_string(&first_response, "x-good-boy").as_deref(),
            Some("true")
        );
        assert!(::std::str::from_utf8(first_response.body().as_ref())?.contains("body=alpha"));

        let mut second_session = TestSession::new(
            "example.test",
            b"/ffi-smoke",
            "POST",
            Some(bytes::Bytes::from_static(b"beta")),
        )?;
        let second_req_ctx = build_request_context(
            &mut second_session,
            b"/ffi-smoke",
            Some(bytes::Bytes::from_static(b"beta")),
            session_id,
            false,
        );

        host.call_module(&second_req_ctx, &params, &mut second_session)
            .await?;

        let second_response = second_session
            .response
            .clone()
            .ok_or("second module call did not write a response")?;

        assert_eq!(second_response.status(), http::StatusCode::OK);
        assert_eq!(
            response_header_string(&second_response, "x-seen-before").as_deref(),
            Some("/ffi-smoke|alpha")
        );
        assert_eq!(
            response_header_string(&second_response, "x-score-before").as_deref(),
            Some("70")
        );
        assert_eq!(
            response_header_string(&second_response, "x-score-after").as_deref(),
            Some("20")
        );
        assert!(::std::str::from_utf8(second_response.body().as_ref())?.contains("body=beta"));

        let stored_seen_key = ksbh_core::storage::module_session_key::ModuleSessionKey::new(
            "dynamic-ffi-smoke:seen",
            session_id,
        );
        let stored_seen = host
            .session_store()
            .get_hot_or_cold_sync(&stored_seen_key)
            .ok_or("session store did not contain persisted `seen` value")?;
        assert_eq!(stored_seen, b"/ffi-smoke|beta".to_vec());

        let metrics_key =
            ksbh_core::storage::module_session_key::ModuleSessionKey::user_session(session_id);
        let stored_metrics = host
            .session_store()
            .get_hot_or_cold_sync(&metrics_key)
            .ok_or("metrics store did not contain persisted score")?;
        let final_score =
            rmp_serde::from_slice::<ksbh_core::metrics::AtomicU64Wrapper>(&stored_metrics)?.load();
        assert_eq!(final_score, 20);
    }

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_injects_proxy_cookie_when_needed()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;

    let session_id = uuid::Uuid::new_v4();
    seed_metrics_score(host, session_id, 50)?;

    let params = build_params();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-cookie",
        "POST",
        Some(bytes::Bytes::from_static(b"cookie-test")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-cookie",
        Some(bytes::Bytes::from_static(b"cookie-test")),
        session_id,
        true,
    );

    host.call_module(&req_ctx, &params, &mut session).await?;

    let response = session
        .response
        .clone()
        .ok_or("module call did not write a response")?;

    assert_eq!(response.status(), http::StatusCode::OK);
    let set_cookie = response
        .headers()
        .get(http::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or("missing proxy session set-cookie header")?;
    assert!(set_cookie.contains("ksbh="));
    assert!(set_cookie.contains("HttpOnly"));

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_returns_module_not_found_for_unloaded_type()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = create_host()?;
    let session_id = uuid::Uuid::new_v4();
    let params = build_params();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-missing",
        "POST",
        Some(bytes::Bytes::from_static(b"missing")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-missing",
        Some(bytes::Bytes::from_static(b"missing")),
        session_id,
        false,
    );

    let error = host
        .call_module(&req_ctx, &params, &mut session)
        .await
        .err()
        .ok_or("expected unloaded module error")?;

    assert_eq!(
        error,
        ksbh_core::modules::abi::module_host::ModuleHostError::ModuleNotFound
    );

    Ok(())
}
