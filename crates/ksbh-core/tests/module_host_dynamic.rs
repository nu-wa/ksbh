struct TestSession {
    request_header: pingora_http::RequestHeader,
    request_body: Option<bytes::Bytes>,
    response: Option<http::Response<bytes::Bytes>>,
    sent: bool,
    client_addr: Option<::std::net::IpAddr>,
    server_addr: Option<::std::net::SocketAddr>,
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
            server_addr: Some(::std::net::SocketAddr::from((
                ::std::net::Ipv4Addr::LOCALHOST,
                8080,
            ))),
        })
    }
}

#[async_trait::async_trait]
impl ksbh_types::prelude::ProxyProviderSession for TestSession {
    fn headers(&self) -> http::request::Parts {
        self.request_header.as_owned_parts()
    }

    fn header_map(&self) -> &http::HeaderMap {
        &self.request_header.headers
    }

    fn get_header(&self, header_name: http::HeaderName) -> Option<&http::header::HeaderValue> {
        self.request_header.headers.get(header_name)
    }

    fn set_request_uri(&mut self, uri: http::Uri) {
        self.request_header.set_uri(uri);
    }

    fn server_addr(&self) -> Option<::std::net::SocketAddr> {
        self.server_addr
    }

    fn response_written(&self) -> bool {
        self.response.is_some()
    }

    fn response_status(&self) -> Option<http::StatusCode> {
        self.response.as_ref().map(|response| response.status())
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
        false,
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

fn set_cookie_values(
    response: &http::Response<bytes::Bytes>,
) -> ::std::vec::Vec<::std::string::String> {
    response
        .headers()
        .get_all(http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok().map(::std::string::String::from))
        .collect()
}

#[tokio::test]
async fn dynamic_module_host_loads_real_cdylib_and_persists_state()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    for _ in 0..dynamic_smoke_loops() {
        let session_id = uuid::Uuid::new_v4();

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
        assert!(::std::str::from_utf8(second_response.body().as_ref())?.contains("body=beta"));
    }

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_injects_proxy_cookie_when_needed()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    let session_id = uuid::Uuid::new_v4();
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
    let set_cookie_values = set_cookie_values(&response);
    assert_eq!(
        set_cookie_values
            .iter()
            .filter(|value| value.contains("ksbh="))
            .count(),
        1
    );
    assert!(
        set_cookie_values
            .iter()
            .any(|value| value.contains("HttpOnly"))
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_does_not_inject_proxy_cookie_when_already_present()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    let session_id = uuid::Uuid::new_v4();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-cookie-present",
        "POST",
        Some(bytes::Bytes::from_static(b"cookie-test")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-cookie-present",
        Some(bytes::Bytes::from_static(b"cookie-test")),
        session_id,
        true,
    );

    host.call_module(&req_ctx, &params, &mut session).await?;

    let response = session
        .response
        .clone()
        .ok_or("module call did not write a response")?;

    let set_cookie_values = set_cookie_values(&response);
    assert_eq!(
        set_cookie_values
            .iter()
            .filter(|value| value.contains("ksbh="))
            .count(),
        1
    );
    assert!(
        set_cookie_values
            .iter()
            .any(|value| value.contains("already-present"))
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_injects_proxy_cookie_when_existing_set_cookie_is_malformed()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    let session_id = uuid::Uuid::new_v4();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-cookie-malformed",
        "POST",
        Some(bytes::Bytes::from_static(b"cookie-test")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-cookie-malformed",
        Some(bytes::Bytes::from_static(b"cookie-test")),
        session_id,
        true,
    );

    host.call_module(&req_ctx, &params, &mut session).await?;

    let response = session
        .response
        .clone()
        .ok_or("module call did not write a response")?;

    let set_cookie_values = set_cookie_values(&response);
    assert_eq!(
        set_cookie_values
            .iter()
            .filter(|value| value.contains("ksbh="))
            .count(),
        1
    );
    assert!(
        set_cookie_values
            .iter()
            .any(|value| value == "not-a-cookie")
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_adds_content_length_when_missing()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    let session_id = uuid::Uuid::new_v4();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-length-missing",
        "POST",
        Some(bytes::Bytes::from_static(b"length-test")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-length-missing",
        Some(bytes::Bytes::from_static(b"length-test")),
        session_id,
        false,
    );

    host.call_module(&req_ctx, &params, &mut session).await?;

    let response = session
        .response
        .clone()
        .ok_or("module call did not write a response")?;

    let body_len = response.body().len().to_string();
    assert_eq!(
        response_header_string(&response, "content-length").as_deref(),
        Some(body_len.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_keeps_existing_content_length_header()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    let session_id = uuid::Uuid::new_v4();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-length-present",
        "POST",
        Some(bytes::Bytes::from_static(b"length-test")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-length-present",
        Some(bytes::Bytes::from_static(b"length-test")),
        session_id,
        false,
    );

    host.call_module(&req_ctx, &params, &mut session).await?;

    let response = session
        .response
        .clone()
        .ok_or("module call did not write a response")?;

    assert_eq!(
        response_header_string(&response, "content-length").as_deref(),
        Some("11")
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_skips_invalid_module_headers_without_failing()
-> Result<(), Box<dyn ::std::error::Error>> {
    let host = shared_host()?;
    let params = build_params();

    let session_id = uuid::Uuid::new_v4();
    let mut session = TestSession::new(
        "example.test",
        b"/ffi-invalid-header",
        "POST",
        Some(bytes::Bytes::from_static(b"header-test")),
    )?;
    let req_ctx = build_request_context(
        &mut session,
        b"/ffi-invalid-header",
        Some(bytes::Bytes::from_static(b"header-test")),
        session_id,
        false,
    );

    host.call_module(&req_ctx, &params, &mut session).await?;

    let response = session
        .response
        .clone()
        .ok_or("module call did not write a response")?;

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response_header_string(&response, "x-valid").as_deref(),
        Some("ok")
    );
    assert!(!response.headers().contains_key("bad header name"));
    let body_len = response.body().len().to_string();
    assert_eq!(
        response_header_string(&response, "content-length").as_deref(),
        Some(body_len.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_reports_loader_failure_when_library_cannot_load()
-> Result<(), Box<dyn ::std::error::Error>> {
    let _ = shared_host()?;
    let host = create_host()?;
    let bogus_path = ::std::path::PathBuf::from("/definitely/not/a/real/module.so");
    let error = host
        .load_module(&bogus_path)
        .err()
        .ok_or("expected load failure")?;

    match error {
        ksbh_core::modules::abi::error::AbiError::ModuleInstanceError(
            ksbh_core::modules::abi::module_instance::ModuleInstanceError::FailedToLoad(_),
        ) => {}
        other => return Err(format!("unexpected loader error: {:?}", other).into()),
    }

    Ok(())
}

#[tokio::test]
async fn dynamic_module_host_returns_module_not_found_for_unloaded_type()
-> Result<(), Box<dyn ::std::error::Error>> {
    let _ = shared_host()?;
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
