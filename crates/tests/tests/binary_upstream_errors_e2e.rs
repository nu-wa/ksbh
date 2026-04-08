async fn read_request_from_stream(
    stream: &mut tokio::net::TcpStream,
) -> Result<(http::Method, String, bytes::Bytes), String> {
    let mut buffer = Vec::new();
    let mut headers_end = None;
    while headers_end.is_none() {
        let mut chunk = [0u8; 1024];
        let read = tokio::io::AsyncReadExt::read(stream, &mut chunk)
            .await
            .map_err(|error| format!("failed to read upstream request headers: {error}"))?;
        if read == 0 {
            return Err("upstream connection closed before request headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
        headers_end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
        if buffer.len() > 64 * 1024 {
            return Err("upstream request headers exceeded 64KB".to_string());
        }
    }

    let header_end = headers_end
        .map(|position| position + 4)
        .ok_or_else(|| "missing upstream request header terminator".to_string())?;
    let header_bytes = &buffer[..header_end];
    let header_text = ::std::str::from_utf8(header_bytes)
        .map_err(|error| format!("upstream request headers were not utf8: {error}"))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing upstream request line".to_string())?;
    let mut request_line_parts = request_line.split_whitespace();
    let method_str = request_line_parts
        .next()
        .ok_or_else(|| format!("invalid upstream request line `{request_line}`"))?;
    let path = request_line_parts
        .next()
        .ok_or_else(|| format!("invalid upstream request line `{request_line}`"))?
        .to_string();
    let method = http::Method::from_bytes(method_str.as_bytes())
        .map_err(|error| format!("invalid upstream request method `{method_str}`: {error}"))?;

    let mut content_length = 0usize;
    let mut transfer_chunked = false;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("invalid content-length `{value}`: {error}"))?;
            }
            if name.trim().eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
            {
                transfer_chunked = true;
            }
        }
    }

    let mut body = buffer[header_end..].to_vec();
    if transfer_chunked {
        let mut payload = Vec::new();
        loop {
            while !body.windows(2).any(|window| window == b"\r\n") {
                let mut chunk = [0u8; 1024];
                let read = tokio::io::AsyncReadExt::read(stream, &mut chunk)
                    .await
                    .map_err(|error| format!("failed to read chunk-size line: {error}"))?;
                if read == 0 {
                    return Err(
                        "upstream connection closed while reading chunk-size line".to_string()
                    );
                }
                body.extend_from_slice(&chunk[..read]);
            }

            let line_end = body
                .windows(2)
                .position(|window| window == b"\r\n")
                .ok_or_else(|| "missing chunk-size delimiter".to_string())?;
            let size_line = std::str::from_utf8(&body[..line_end])
                .map_err(|error| format!("chunk-size line was not utf8: {error}"))?;
            let size = usize::from_str_radix(size_line.trim(), 16)
                .map_err(|error| format!("invalid chunk-size `{size_line}`: {error}"))?;
            body.drain(..line_end + 2);

            if size == 0 {
                while body.len() < 2 {
                    let mut chunk = [0u8; 16];
                    let read = tokio::io::AsyncReadExt::read(stream, &mut chunk)
                        .await
                        .map_err(|error| format!("failed to read chunked terminator: {error}"))?;
                    if read == 0 {
                        break;
                    }
                    body.extend_from_slice(&chunk[..read]);
                }
                break;
            }

            while body.len() < size + 2 {
                let mut chunk = [0u8; 2048];
                let read = tokio::io::AsyncReadExt::read(stream, &mut chunk)
                    .await
                    .map_err(|error| format!("failed to read chunk payload: {error}"))?;
                if read == 0 {
                    return Err(
                        "upstream connection closed while reading chunk payload".to_string()
                    );
                }
                body.extend_from_slice(&chunk[..read]);
            }

            payload.extend_from_slice(&body[..size]);
            body.drain(..size + 2);
        }

        body = payload;
    } else if body.len() < content_length {
        let remaining = content_length - body.len();
        let mut rest = vec![0u8; remaining];
        tokio::io::AsyncReadExt::read_exact(stream, &mut rest)
            .await
            .map_err(|error| format!("failed to read upstream request body: {error}"))?;
        body.extend_from_slice(&rest);
    }

    Ok((method, path, bytes::Bytes::from(body)))
}

async fn spawn_upstream_once<F>(
    handler: F,
) -> Result<(u16, tokio::sync::oneshot::Receiver<Result<(), String>>), String>
where
    F: FnOnce(
            tokio::net::TcpStream,
        )
            -> ::std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
        + Send
        + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| format!("failed to bind upstream listener: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("failed to get upstream listener addr: {error}"))?
        .port();
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

    tokio::spawn(async move {
        let result = match listener.accept().await {
            Ok((stream, _)) => handler(stream).await,
            Err(error) => Err(format!("failed to accept upstream connection: {error}")),
        };
        let _ = tx.send(result);
    });

    Ok((port, rx))
}

async fn assert_upstream_result(
    rx: tokio::sync::oneshot::Receiver<Result<(), String>>,
    fixture: &tests::binary::BinaryFixture,
) {
    match tokio::time::timeout(tokio::time::Duration::from_secs(20), rx).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => panic!(
            "upstream assertion failed: {error}\nlogs:\n{}",
            fixture.logs()
        ),
        Ok(Err(error)) => panic!(
            "upstream channel closed unexpectedly: {error}\nlogs:\n{}",
            fixture.logs()
        ),
        Err(_) => panic!(
            "timed out waiting for upstream assertion\nlogs:\n{}",
            fixture.logs()
        ),
    }
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

fn repo_root() -> ::std::path::PathBuf {
    let manifest_dir = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(::std::path::Path::to_path_buf)
        .unwrap_or(manifest_dir)
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
            let entry = entry
                .map_err(|error| format!("failed to iterate module artifact directory: {error}"))?;
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

#[tokio::test]
async fn binary_file_provider_unreachable_upstream_returns_internal_502_page() {
    let routing_yaml = r#"
ingresses:
  - name: upstream-failure-ingress
    host: upstream-failure.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: 1
"#;

    let mut fixture = tests::binary::BinaryFixture::new("upstream-502", routing_yaml)
        .expect("failed to create binary fixture");
    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    let request_start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_secs(20);

    let response = loop {
        assert!(
            request_start.elapsed() < timeout,
            "timed out waiting for upstream 502 response\nlogs:\n{}",
            fixture.logs()
        );

        match tests::binary::get_with_host(
            &client,
            &fixture.http_base_addr(),
            "/",
            "upstream-failure.test.local",
        )
        .await
        {
            Ok(response) if response.status() == reqwest::StatusCode::BAD_GATEWAY => {
                break response;
            }
            Ok(_) | Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            }
        }
    };

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response
        .text()
        .await
        .unwrap_or_else(|error| panic!("failed to read upstream 502 response body: {error}"));

    assert!(
        content_type.starts_with("text/html"),
        "expected internal 502 page content-type, got `{content_type}`\nbody=`{body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert!(
        body.contains("502") && body.contains("Bad Gateway"),
        "expected internal 502 page body for upstream failure, got `{body}`\nlogs:\n{}",
        fixture.logs()
    );
}

#[tokio::test]
async fn binary_file_provider_forwards_body_after_requires_body_module_reads_it() {
    let expected_payload =
        "{\"component\":\"ak-stage-identification\",\"uid_field\":\"authentik@yannis.codes\"}";
    let (upstream_port, upstream_result) = spawn_upstream_once(move |mut stream| {
        let expected_payload = expected_payload.to_string();
        Box::pin(async move {
            let mut validation_error = None;
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(3),
                read_request_from_stream(&mut stream),
            )
            .await
            {
                Err(_) => {
                    validation_error =
                        Some("timed out waiting for upstream request bytes".to_string());
                }
                Ok(result) => match result {
                Ok((method, path, body)) => {
                    if method != http::Method::POST {
                        validation_error = Some(format!("expected upstream method POST, got {method}"));
                    } else if path != "/submit" {
                        validation_error = Some(format!("expected upstream path /submit, got {path}"));
                    } else if body.as_ref() != expected_payload.as_bytes() {
                        validation_error = Some(format!(
                            "unexpected upstream body `{}`",
                            String::from_utf8_lossy(body.as_ref())
                        ));
                    }
                }
                Err(error) => {
                    validation_error = Some(error);
                }
                },
            }

            let (status_line, response_body) = if let Some(error) = &validation_error {
                ("HTTP/1.1 400 Bad Request", error.as_str())
            } else {
                ("HTTP/1.1 200 OK", "ok")
            };
            let response = format!(
                "{status_line}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            tokio::io::AsyncWriteExt::write_all(
                &mut stream,
                response.as_bytes(),
            )
            .await
            .map_err(|error| format!("failed to write upstream success response: {error}"))?;

            if let Some(error) = validation_error {
                return Err(error);
            }

            Ok(())
        })
    })
    .await
    .unwrap_or_else(|error| panic!("failed to spawn upstream server: {error}"));

    let routing_yaml = format!(
        r#"
modules:
  - name: body-reader
    type: robotsdottxt
    weight: 10
    global: true
    requires_body: true
    config:
      content: "User-agent: *\nDisallow: /"
ingresses:
  - name: body-forward-ingress
    host: body-forward.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: {upstream_port}
"#
    );

    let mut fixture = tests::binary::BinaryFixture::new("body-forward", routing_yaml.as_str())
        .expect("failed to create binary fixture");
    let module_artifact = find_compiled_module_artifact("robots_txt").unwrap_or_else(|error| {
        panic!("{error}; run `cargo build -p robots-txt --manifest-path crates/Cargo.toml` first")
    });
    let module_file_name = module_artifact.file_name().unwrap_or_else(|| {
        panic!(
            "module artifact path has no file name: {:?}",
            module_artifact
        )
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
            "internal health check failed before body-forward test: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let response = client
        .post(format!("{}/submit", fixture.http_base_addr()))
        .header(reqwest::header::HOST, "body-forward.test.local")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(expected_payload.to_string())
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to send proxied post request: {error}\nlogs:\n{}",
                fixture.logs()
            )
        });

    let status = response.status();
    let body = response.text().await.unwrap_or_else(|error| {
        panic!(
            "failed to read proxied post response body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "expected upstream post to succeed, got status {status} body `{body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert_eq!(
        body,
        "ok",
        "expected upstream response body `ok`, got `{body}`\nlogs:\n{}",
        fixture.logs()
    );

    assert_upstream_result(upstream_result, &fixture).await;
}

#[tokio::test]
async fn binary_file_provider_replaces_empty_upstream_error_body_with_internal_html() {
    let (upstream_port, upstream_result) = spawn_upstream_once(|mut stream| {
        Box::pin(async move {
            let _ = read_request_from_stream(&mut stream).await?;
            tokio::io::AsyncWriteExt::write_all(
                &mut stream,
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .map_err(|error| format!("failed to write upstream empty error response: {error}"))?;
            Ok(())
        })
    })
    .await
    .unwrap_or_else(|error| panic!("failed to spawn upstream server: {error}"));

    let routing_yaml = format!(
        r#"
ingresses:
  - name: upstream-empty-error-ingress
    host: upstream-empty-error.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: {upstream_port}
"#
    );

    let mut fixture =
        tests::binary::BinaryFixture::new("upstream-empty-error", routing_yaml.as_str())
            .expect("failed to create binary fixture");
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
            "internal health check failed before empty-error test: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let response = client
        .get(format!("{}/", fixture.http_base_addr()))
        .header(reqwest::header::HOST, "upstream-empty-error.test.local")
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to send proxied request: {error}\nlogs:\n{}",
                fixture.logs()
            )
        });

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|error| {
        panic!(
            "failed to read proxied response body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    assert_eq!(
        status,
        reqwest::StatusCode::BAD_GATEWAY,
        "expected 502 status, got {status}\nbody=`{body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert!(
        content_type.starts_with("text/html"),
        "expected fallback html content-type, got `{content_type}`\nbody=`{body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert!(
        body.contains("Bad Gateway"),
        "expected fallback html error body, got `{body}`\nlogs:\n{}",
        fixture.logs()
    );

    assert_upstream_result(upstream_result, &fixture).await;
}

#[tokio::test]
async fn binary_file_provider_keeps_non_empty_upstream_error_body() {
    let upstream_body = "upstream said no";
    let (upstream_port, upstream_result) = spawn_upstream_once(move |mut stream| {
        let upstream_body = upstream_body.to_string();
        Box::pin(async move {
            let _ = read_request_from_stream(&mut stream).await?;
            let response = format!(
                "HTTP/1.1 502 Bad Gateway\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
                upstream_body.len(),
                upstream_body
            );
            tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                .await
                .map_err(|error| format!("failed to write upstream non-empty error response: {error}"))?;
            Ok(())
        })
    })
    .await
    .unwrap_or_else(|error| panic!("failed to spawn upstream server: {error}"));

    let routing_yaml = format!(
        r#"
ingresses:
  - name: upstream-non-empty-error-ingress
    host: upstream-non-empty-error.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: {upstream_port}
"#
    );

    let mut fixture =
        tests::binary::BinaryFixture::new("upstream-non-empty-error", routing_yaml.as_str())
            .expect("failed to create binary fixture");
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
            "internal health check failed before non-empty-error test: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let response = client
        .get(format!("{}/", fixture.http_base_addr()))
        .header(reqwest::header::HOST, "upstream-non-empty-error.test.local")
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to send proxied request: {error}\nlogs:\n{}",
                fixture.logs()
            )
        });

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|error| {
        panic!(
            "failed to read proxied response body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    assert_eq!(
        status,
        reqwest::StatusCode::BAD_GATEWAY,
        "expected 502 status, got {status}\nbody=`{body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert_eq!(
        content_type, "text/plain",
        "expected upstream content-type to be preserved, got `{content_type}`\nbody=`{body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert_eq!(
        body,
        upstream_body,
        "expected upstream body to be preserved, got `{body}`\nlogs:\n{}",
        fixture.logs()
    );

    assert_upstream_result(upstream_result, &fixture).await;
}
