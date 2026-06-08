use sha2::Digest;

// ── Shared helpers ────────────────────────────────────────────────────

fn repo_root() -> ::std::path::PathBuf {
    let manifest_dir = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(::std::path::Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

fn module_library_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

fn find_compiled_module_artifact(module_stem: &str) -> Result<::std::path::PathBuf, String> {
    let ext = module_library_extension();
    let target_debug = repo_root().join("crates").join("target").join("debug");
    let prefixes = [format!("lib{module_stem}"), module_stem.to_string()];
    let mut candidates = Vec::new();

    for dir in [target_debug.clone(), target_debug.join("deps")] {
        let entries = ::std::fs::read_dir(&dir).map_err(|error| {
            format!("failed to read module artifact directory {:?}: {error}", dir)
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("failed to iterate module artifact directory: {error}")
            })?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_ext) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if file_ext != ext {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if prefixes.iter().any(|prefix| file_name.starts_with(prefix)) {
                candidates.push(path);
            }
        }
    }

    candidates.sort();
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| format!("could not find compiled module artifact for `{module_stem}`"))
}

// ── Wait helper ────────────────────────────────────────────────────────

async fn wait_for_status_with_host(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
) -> Result<reqwest::Response, String> {
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_secs(30);
    let mut last_error = String::new();

    while start.elapsed() < timeout {
        match client
            .get(format!("{base_addr}{path}"))
            .header(reqwest::header::HOST, host)
            .send()
            .await
        {
            Ok(response) if response.status() == expected_status => return Ok(response),
            Ok(response) => {
                last_error = format!(
                    "unexpected status {} while waiting for {}{} (host: {})",
                    response.status(),
                    base_addr,
                    path,
                    host
                );
            }
            Err(error) => {
                last_error = format!(
                    "request failed while waiting for {}{} (host: {}): {}",
                    base_addr, path, host, error
                );
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    Err(last_error)
}

// ── Retry helper for module loading race ─────────────────────────────

async fn get_with_retry(
    client: &reqwest::Client,
    url: &str,
    host: &str,
) -> (reqwest::StatusCode, String) {
    let mut last_status = reqwest::StatusCode::default();
    let mut last_body = String::new();
    for _ in 0..20 {
        let response = client
            .get(url)
            .header(reqwest::header::HOST, host)
            .send()
            .await
            .expect("request");
        last_status = response.status();
        last_body = response.text().await.unwrap_or_default();
        if !is_transient_module_loading_failure(last_status, &last_body) {
            return (last_status, last_body);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    (last_status, last_body)
}

fn is_transient_module_loading_failure(status: reqwest::StatusCode, body: &str) -> bool {
    matches!(
        status,
        reqwest::StatusCode::INTERNAL_SERVER_ERROR | reqwest::StatusCode::BAD_GATEWAY
    ) && (body.contains("module ") || body.contains("Bad Gateway"))
}

#[tokio::test]
async fn unauthenticated_request_redirects_to_oidc_provider() {
    let oidc_server = httpmock::MockServer::start();
    let issuer = oidc_server.base_url();
    let client_id = "test-client-id";

    // Mock OIDC discovery
    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/.well-known/openid-configuration");
        then.status(200).json_body(serde_json::json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "jwks_uri": format!("{issuer}/jwks"),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        }));
    });

    // Mock JWKS (minimal, just enough for the provider to accept)
    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/jwks");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"keys":[]}"#);
    });

    // Mock authorization endpoint (redirects back to ksbh)
    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/authorize");
        then.status(302).header(
            "Location",
            "http://oidc.test.local/_ksbh_internal/oidc?code=auth-code-123&state=some-state",
        );
    });

    let routing_yaml = format!(
        r#"
modules:
  - name: test-oidc
    type: oidc
    weight: 10
    global: true
    requires_body: false
    config:
      issuer_url: "{issuer}"
      client_id: "{client_id}"
      client_secret: "test-client-secret"
ingresses:
  - name: oidc-test-ingress
    host: oidc.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 0
"#
    );

    let mut fixture =
        tests::binary::BinaryFixture::new("oidc-redirect", routing_yaml.as_str())
            .expect("failed to create binary fixture");

    let module_artifact = find_compiled_module_artifact("oidc").unwrap_or_else(|error| {
        panic!("{error}; run `cargo build -p oidc --manifest-path crates/Cargo.toml` first")
    });
    let module_file_name = module_artifact.file_name().unwrap_or_else(|| {
        panic!("module artifact path has no file name: {:?}", module_artifact)
    });
    ::std::fs::copy(
        &module_artifact,
        fixture.modules_dir().join(module_file_name),
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to copy module artifact {:?} into fixture modules dir: {error}",
            module_artifact
        )
    });

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    tests::binary::wait_for_status(
        &client,
        &fixture.internal_base_addr(),
        "/healthz",
        reqwest::StatusCode::OK,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "internal health check failed: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    // Unauthenticated request → should redirect to OIDC provider's authorize endpoint
    let response = wait_for_status_with_host(
        &client,
        &fixture.http_base_addr(),
        "/dashboard",
        "oidc.test.local",
        reqwest::StatusCode::FOUND,
    )
    .await
    .unwrap_or_else(|error| {
        panic!("expected 302 redirect: {error}\nlogs:\n{}", fixture.logs())
    });

    let location = response
        .headers()
        .get(http::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
        .unwrap_or_default();

    assert!(
        location.contains(&format!("{issuer}/authorize")),
        "redirect should point to OIDC authorize endpoint, got: {location}\nlogs:\n{}",
        fixture.logs()
    );
}

// ── Test: favicon bypasses OIDC ──────────────────────────────────────

#[tokio::test]
async fn favicon_request_bypasses_oidc() {
    let oidc_server = httpmock::MockServer::start();
    let issuer = oidc_server.base_url();

    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/.well-known/openid-configuration");
        then.status(200).json_body(serde_json::json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "jwks_uri": format!("{issuer}/jwks"),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        }));
    });

    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/jwks");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"keys":[]}"#);
    });

    let routing_yaml = format!(
        r#"
modules:
  - name: test-oidc-favicon
    type: oidc
    weight: 10
    global: true
    requires_body: false
    config:
      issuer_url: "{issuer}"
      client_id: "test-client-id"
      client_secret: "test-client-secret"
ingresses:
  - name: oidc-favicon-ingress
    host: oidc-favicon.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 0
"#
    );

    let mut fixture =
        tests::binary::BinaryFixture::new("oidc-favicon", routing_yaml.as_str())
            .expect("failed to create binary fixture");

    let module_artifact = find_compiled_module_artifact("oidc").unwrap_or_else(|error| {
        panic!("{error}; run `cargo build -p oidc --manifest-path crates/Cargo.toml` first")
    });
    let module_file_name = module_artifact.file_name().unwrap_or_else(|| {
        panic!("module artifact path has no file name: {:?}", module_artifact)
    });
    ::std::fs::copy(
        &module_artifact,
        fixture.modules_dir().join(module_file_name),
    )
    .unwrap_or_else(|error| {
        panic!("failed to copy oidc module artifact into fixture modules dir: {error}")
    });

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    tests::binary::wait_for_status(
        &client,
        &fixture.internal_base_addr(),
        "/healthz",
        reqwest::StatusCode::OK,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "internal health check failed: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    // Favicon request should NOT redirect (Pass, proxy to upstream)
    let response = wait_for_status_with_host(
        &client,
        &fixture.http_base_addr(),
        "/favicon.ico",
        "oidc-favicon.test.local",
        reqwest::StatusCode::BAD_GATEWAY,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "favicon should not redirect (expected 502 from broken upstream): {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    assert_eq!(
        response.status(),
        reqwest::StatusCode::BAD_GATEWAY,
        "favicon should reach upstream (502 = no upstream, 302 would mean auth redirect)\nlogs:\n{}",
        fixture.logs()
    );
}

// ── Test: missing code parameter ─────────────────────────────────────

#[tokio::test]
async fn callback_missing_code_is_handled() {
    let oidc_server = httpmock::MockServer::start();
    let issuer = oidc_server.base_url();

    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/.well-known/openid-configuration");
        then.status(200).json_body(serde_json::json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "jwks_uri": format!("{issuer}/jwks"),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        }));
    });

    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/jwks");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(r#"{"keys":[]}"#);
    });

    // Mock authorization endpoint (for the re-auth redirect flow)
    oidc_server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/authorize");
        then.status(302).header(
            "Location",
            "http://oidc-missing.test.local/_ksbh_internal/oidc?code=auth-code-999&state=some-state",
        );
    });

    let routing_yaml = format!(
        r#"
modules:
  - name: test-oidc-missing-code
    type: oidc
    weight: 10
    global: true
    requires_body: false
    config:
      issuer_url: "{issuer}"
      client_id: "test-client-id"
      client_secret: "test-client-secret"
ingresses:
  - name: oidc-missing-code-ingress
    host: oidc-missing.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 0
"#
    );

    let mut fixture =
        tests::binary::BinaryFixture::new("oidc-missing-code", routing_yaml.as_str())
            .expect("failed to create binary fixture");

    let module_artifact = find_compiled_module_artifact("oidc").unwrap_or_else(|error| {
        panic!("{error}; run `cargo build -p oidc --manifest-path crates/Cargo.toml` first")
    });
    let module_file_name = module_artifact.file_name().unwrap_or_else(|| {
        panic!("module artifact path has no file name: {:?}", module_artifact)
    });
    ::std::fs::copy(
        &module_artifact,
        fixture.modules_dir().join(module_file_name),
    )
    .unwrap_or_else(|error| {
        panic!("failed to copy oidc module artifact into fixture modules dir: {error}")
    });

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    tests::binary::wait_for_status(
        &client,
        &fixture.internal_base_addr(),
        "/healthz",
        reqwest::StatusCode::OK,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "internal health check failed: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    // Callback with missing code → should 400 or redirect for re-auth (retry for module loading)
    let (status, body) = get_with_retry(
        &client,
        &format!("{}/_ksbh_internal/oidc?state=some-state", fixture.http_base_addr()),
        "oidc-missing.test.local",
    ).await;
    assert!(
        status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::FOUND,
        "expected 400 (missing code) or 302 (re-auth redirect), got {} body={}\nlogs:\n{}",
        status.as_u16(),
        &body[..body.len().min(500)],
        fixture.logs()
    );
}
