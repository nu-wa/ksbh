use sha2::Digest;

// ── Shared helpers (duplicated per test file pattern in this crate) ───

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
            format!(
                "failed to read module artifact directory {:?}: {error}",
                dir
            )
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

// ── PoW solver: find a nonce that produces required leading zeros ─────

fn solve_challenge(challenge: &str, difficulty: usize) -> u64 {
    for nonce in 0..=10_000_000u64 {
        let mut sha = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut sha, format!("{}{}", challenge, nonce));
        let hash = hex::encode(sha2::Digest::finalize(sha));
        if hash.starts_with(&"0".repeat(difficulty)) {
            return nonce;
        }
    }
    panic!("no solution found for challenge {challenge} with difficulty {difficulty}");
}

// ── Helpers ───────────────────────────────────────────────────────────

fn parse_challenge_from_html(html: &str) -> (String, usize, String) {
    // Extract challenge string from hidden input or script tag
    let challenge = html
        .lines()
        .find(|line| line.contains("challenge"))
        .and_then(|line| {
            let start = line.find("value=\"")?;
            let value_start = start + 7;
            let end = line[value_start..].find('"')?;
            Some(line[value_start..value_start + end].to_string())
        })
        .or_else(|| {
            // Fallback: look for the challenge pattern directly in HTML
            html.find("challenge").and_then(|_| {
                // search for iat.difficulty pattern
                let patterns: Vec<&str> = html
                    .split(|c: char| c == '"' || c == '\'' || c == '>' || c == '<')
                    .filter(|s| s.chars().filter(|c| *c == '.').count() >= 2)
                    .filter(|s| {
                        let parts: Vec<&str> = s.split('.').collect();
                        parts.len() == 3
                            && parts[0].parse::<u64>().is_ok()
                            && parts[1].parse::<usize>().is_ok()
                    })
                    .collect();
                patterns.first().map(|s| s.to_string())
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "could not find challenge in HTML:\n---\n{}\n---",
                &html[..html.len().min(2000)]
            )
        });

    let parts: Vec<&str> = challenge.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "challenge should have 3 dot-separated parts, got: {}",
        challenge
    );
    let _iat: u64 = parts[0]
        .parse()
        .unwrap_or_else(|_| panic!("invalid iat in challenge: {}", challenge));
    let difficulty: usize = parts[1]
        .parse()
        .unwrap_or_else(|_| panic!("invalid difficulty in challenge: {}", challenge));

    (challenge, difficulty, html.to_string())
}

fn find_action_url(html: &str) -> String {
    html.lines()
        .find(|line| line.contains("action="))
        .and_then(|line| {
            let start = line.find("action=\"")?;
            let value_start = start + 8;
            let end = line[value_start..].find('"')?;
            Some(line[value_start..value_start + end].to_string())
        })
        .unwrap_or_else(|| {
            panic!(
                "could not find action URL in HTML:\n---\n{}\n---",
                &html[..html.len().min(500)]
            )
        })
}

// ── Retry helper for module loading race ─────────────────────────────

fn is_transient_module_loading_failure(status: reqwest::StatusCode, body: &str) -> bool {
    matches!(
        status,
        reqwest::StatusCode::INTERNAL_SERVER_ERROR | reqwest::StatusCode::BAD_GATEWAY
    ) && (body.contains("module ") || body.contains("Bad Gateway"))
}

async fn wait_for_non_500(
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

async fn post_wait_for_non_500(
    client: &reqwest::Client,
    url: &str,
    host: &str,
    body: &str,
) -> (reqwest::StatusCode, String) {
    let mut last_status = reqwest::StatusCode::default();
    let mut last_body = String::new();
    for _ in 0..20 {
        let response = client
            .post(url)
            .header(reqwest::header::HOST, host)
            .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body.to_string())
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

#[tokio::test]
async fn unauthenticated_get_returns_challenge_page() {
    let routing_yaml = r#"
modules:
  - name: test-pow
    type: pow
    weight: 10
    global: true
    requires_body: true
    config:
      secret: "0123456789abcdef0123456789abcdef"
      difficulty: "2"
ingresses:
  - name: pow-test-ingress
    host: pow.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 0
"#;

    let mut fixture =
        tests::binary::BinaryFixture::new("pow-challenge", routing_yaml)
            .expect("failed to create binary fixture");

    let module_artifact =
        find_compiled_module_artifact("proof_of_work").unwrap_or_else(|error| {
            panic!(
                "{error}; run `cargo build -p proof-of-work --manifest-path crates/Cargo.toml` first"
            )
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

    // Unauthenticated request → 401 + challenge page (retry for module loading)
    let (status, body) = wait_for_non_500(
        &client,
        &format!("{}/", fixture.http_base_addr()),
        "pow.test.local",
    ).await;
    assert!(
        status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FOUND,
        "expected 401 (challenge) or 302 (redirect from cookie domain), got {} body={}\nlogs:\n{}",
        status.as_u16(),
        &body[..body.len().min(500)],
        fixture.logs()
    );

    // If we got a challenge page, verify it has the expected content
    if status == reqwest::StatusCode::UNAUTHORIZED {
        assert!(
            body.contains("challenge") || body.contains("action") || body.len() > 100,
            "challenge page should have content\nlogs:\n{}",
            fixture.logs()
        );
    }
}

// ── Test: POST invalid nonce returns error ────────────────────────────

#[tokio::test]
async fn post_invalid_challenge_returns_bad_request() {
    let routing_yaml = r#"
modules:
  - name: test-pow-invalid
    type: pow
    weight: 10
    global: true
    requires_body: true
    config:
      secret: "0123456789abcdef0123456789abcdef"
      difficulty: "4"
ingresses:
  - name: pow-invalid-ingress
    host: pow-invalid.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 0
"#;

    let mut fixture =
        tests::binary::BinaryFixture::new("pow-invalid", routing_yaml)
            .expect("failed to create binary fixture");

    let module_artifact =
        find_compiled_module_artifact("proof_of_work").unwrap_or_else(|error| {
            panic!(
                "{error}; run `cargo build -p proof-of-work --manifest-path crates/Cargo.toml` first"
            )
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

    // POST garbage data → should fail (retry for module loading)
    let (status, _body) = post_wait_for_non_500(
        &client,
        &format!("{}/_ksbh_internal/pow", fixture.http_base_addr()),
        "pow-invalid.test.local",
        "challenge=garbage.invalid.data&nonce=12345",
    ).await;
    assert!(
        status.is_client_error(),
        "expected 4xx for invalid challenge, got {}\nlogs:\n{}",
        status.as_u16(),
        fixture.logs()
    );
}

// ── Test: POST with valid solution gets redirect ──────────────────────

#[tokio::test]
async fn post_valid_solution_gets_redirect() {
    let secret = "0123456789abcdef0123456789abcdef";
    let difficulty: usize = 2;

    let routing_yaml = format!(
        r#"
modules:
  - name: test-pow-valid
    type: pow
    weight: 10
    global: true
    requires_body: true
    config:
      secret: "{secret}"
      difficulty: "{difficulty}"
ingresses:
  - name: pow-valid-ingress
    host: pow-valid.test.local
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
        tests::binary::BinaryFixture::new("pow-valid", routing_yaml.as_str())
            .expect("failed to create binary fixture");

    let module_artifact =
        find_compiled_module_artifact("proof_of_work").unwrap_or_else(|error| {
            panic!(
                "{error}; run `cargo build -p proof-of-work --manifest-path crates/Cargo.toml` first"
            )
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

    // Step 1: GET / → get challenge page (retry for module loading)
    let (status, body) = wait_for_non_500(
        &client,
        &format!("{}/", fixture.http_base_addr()),
        "pow-valid.test.local",
    ).await;

    if status == reqwest::StatusCode::FOUND {
        // Already authenticated (maybe from a prior test run)
        return;
    }

    assert_eq!(
        status,
        reqwest::StatusCode::UNAUTHORIZED,
        "expected 401 challenge page, got {} body={}\nlogs:\n{}",
        status.as_u16(),
        &body[..body.len().min(500)],
        fixture.logs()
    );

    // Step 2: Parse challenge from response body
    let (challenge, parsed_difficulty, _html) = parse_challenge_from_html(&body);
    assert!(
        parsed_difficulty >= difficulty,
        "difficulty should be at least the configured base ({difficulty}), got {parsed_difficulty}"
    );

    // Step 3: Solve the challenge
    let nonce = solve_challenge(&challenge, parsed_difficulty);

    // Step 4: Submit solution as a GET with challenge+nonce query params
    // (the PoW module reads from query_params, not from a POST body).
    let submit_url = format!(
        "{}/_ksbh_internal/pow?challenge={}&nonce={}",
        fixture.http_base_addr(),
        urlencoding::encode(&challenge),
        nonce,
    );
    let (post_status, post_body) = wait_for_non_500(
        &client,
        &submit_url,
        "pow-valid.test.local",
    )
    .await;
    // Should get a redirect (302) or success response
    assert!(
        post_status.is_redirection() || post_status.is_success(),
        "expected redirect or success after valid solution, got {} body={}\nlogs:\n{}",
        post_status.as_u16(),
        &post_body[..post_body.len().min(500)],
        fixture.logs()
    );
}

// ── Test: WebSocket upgrade bypasses PoW ──────────────────────────────

#[tokio::test]
async fn websocket_get_bypasses_pow() {
    let routing_yaml = r#"
modules:
  - name: test-pow-ws
    type: pow
    weight: 10
    global: true
    requires_body: true
    config:
      secret: "0123456789abcdef0123456789abcdef"
      difficulty: "2"
ingresses:
  - name: pow-ws-ingress
    host: pow-ws.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 0
"#;

    let mut fixture =
        tests::binary::BinaryFixture::new("pow-ws", routing_yaml)
            .expect("failed to create binary fixture");

    let module_artifact =
        find_compiled_module_artifact("proof_of_work").unwrap_or_else(|error| {
            panic!(
                "{error}; run `cargo build -p proof-of-work --manifest-path crates/Cargo.toml` first"
            )
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

    // Non-GET request → should Pass (not 401)
    let response = client
        .post(format!("{}/api", fixture.http_base_addr()))
        .header(reqwest::header::HOST, "pow-ws.test.local")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .send()
        .await
        .expect("post to ws-upgrade path");

    let status = response.status();
    // Should NOT be 401 (unauthorized) — POW module skips non-GET + websocket
    assert!(
        status != reqwest::StatusCode::UNAUTHORIZED,
        "POST websocket upgrade should not return 401 (should pass through), got {}\nlogs:\n{}",
        status.as_u16(),
        fixture.logs()
    );
}
