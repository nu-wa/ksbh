mod common {
    pub(super) async fn read_request_from_stream(
        stream: &mut tokio::net::TcpStream,
    ) -> Result<(http::Method, String, Vec<(String, String)>, bytes::Bytes), String> {
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

        let mut headers = Vec::new();
        let mut content_length = 0usize;
        let mut transfer_chunked = false;
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_string();
            let value = value.trim().to_string();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid content-length `{value}`: {error}"))?;
            }
            if name.eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
            {
                transfer_chunked = true;
            }
            headers.push((name, value));
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
                            .map_err(|error| {
                                format!("failed to read chunked terminator: {error}")
                            })?;
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

        Ok((method, path, headers, bytes::Bytes::from(body)))
    }
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[tokio::test]
async fn binary_https_authentik_like_flow_preserves_session_and_post_body() {
    let expected_path = "/api/v3/flows/executor/default-authentication-flow/?query=next%3D%252F";
    let expected_body =
        "{\"component\":\"ak-stage-identification\",\"uid_field\":\"akadmin\",\"password\":\"insecure-admin-password\"}";
    let issued_session = "issued-session-token";

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|error| panic!("failed to bind auth-like upstream listener: {error}"));
    let upstream_port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("failed to get auth-like upstream addr: {error}"))
        .port();
    let (result_tx, result_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let expected_body_owned = expected_body.to_string();
    let expected_path_owned = expected_path.to_string();
    let issued_session_owned = issued_session.to_string();

    tokio::spawn(async move {
        let result = async {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|error| format!("failed to accept upstream connection: {error}"))?;

            let (method, path, headers, body) = common::read_request_from_stream(&mut stream).await?;
            if method != http::Method::GET {
                return Err(format!("expected first request method GET, got {method}"));
            }
            if path != expected_path_owned {
                return Err(format!("expected first request path {expected_path_owned}, got {path}"));
            }
            if !body.is_empty() {
                return Err(format!(
                    "expected first request body to be empty, got `{}`",
                    String::from_utf8_lossy(body.as_ref())
                ));
            }
            if header_value(&headers, "x-forwarded-proto") != Some("https") {
                return Err(format!(
                    "expected x-forwarded-proto=https on first request, got {:?}",
                    header_value(&headers, "x-forwarded-proto")
                ));
            }
            let host_header = header_value(&headers, "host")
                .ok_or_else(|| "expected first request to include host header".to_string())?;
            if host_header != format!("127.0.0.1:{upstream_port}").as_str()
                && host_header != "auth-flow.test.local"
                && !host_header.starts_with("auth-flow.test.local:")
            {
                return Err(format!(
                    "expected host to be backend target or preserved external host, got {host_header}",
                ));
            }
            let forwarded_host = header_value(&headers, "x-forwarded-host")
                .ok_or_else(|| "expected first request to include x-forwarded-host header".to_string())?;
            if forwarded_host != "auth-flow.test.local"
                && !forwarded_host.starts_with("auth-flow.test.local:")
            {
                return Err(format!(
                    "expected x-forwarded-host auth-flow.test.local[:port] on first request, got {forwarded_host}",
                ));
            }

            let get_response_body = "{\"component\":\"ak-stage-identification\",\"password_fields\":false}";
            let get_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nSet-Cookie: authentik_session={}; HttpOnly; Path=/; SameSite=None; Secure\r\nConnection: keep-alive\r\n\r\n{}",
                get_response_body.len(),
                issued_session_owned,
                get_response_body
            );
            tokio::io::AsyncWriteExt::write_all(&mut stream, get_response.as_bytes())
                .await
                .map_err(|error| format!("failed to write first upstream response: {error}"))?;

            let (method, path, headers, body) = common::read_request_from_stream(&mut stream).await?;
            if method != http::Method::POST {
                return Err(format!("expected second request method POST, got {method}"));
            }
            if path != expected_path_owned {
                return Err(format!("expected second request path {expected_path_owned}, got {path}"));
            }
            if header_value(&headers, "cookie")
                != Some(format!("authentik_session={issued_session_owned}").as_str())
            {
                return Err(format!(
                    "expected second request cookie authentik_session={issued_session_owned}, got {:?}",
                    header_value(&headers, "cookie")
                ));
            }
            if body.as_ref() != expected_body_owned.as_bytes() {
                return Err(format!(
                    "unexpected second request body `{}`",
                    String::from_utf8_lossy(body.as_ref())
                ));
            }

            let post_response_body = "{\"component\":\"ak-stage-password\",\"password_fields\":true}";
            let post_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                post_response_body.len(),
                post_response_body
            );
            tokio::io::AsyncWriteExt::write_all(&mut stream, post_response.as_bytes())
                .await
                .map_err(|error| format!("failed to write second upstream response: {error}"))?;

            Ok(())
        }
        .await;

        let _ = result_tx.send(result);
    });

    let routing_yaml = format!(
        r#"
ingresses:
  - name: auth-flow-ingress
    host: auth-flow.test.local
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: 127.0.0.1
          port: {upstream_port}
"#
    );

    let mut fixture = tests::binary::BinaryFixture::new("auth-flow", routing_yaml.as_str())
        .expect("failed to create binary fixture");
    fixture.start().expect("failed to start ksbh binary");

    let resolved_https_addr = fixture
        .https_base_addr()
        .trim_start_matches("https://")
        .parse::<::std::net::SocketAddr>()
        .unwrap_or_else(|error| {
            panic!(
                "failed to parse fixture https socket address `{}`: {error}",
                fixture.https_base_addr()
            )
        });
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(tokio::time::Duration::from_secs(5))
        .resolve("auth-flow.test.local", resolved_https_addr)
        .build()
        .expect("failed to create reqwest client for auth-like flow test");
    let proxied_base_addr = format!(
        "https://auth-flow.test.local:{}",
        resolved_https_addr.port()
    );
    tests::binary::wait_for_status(
        &client,
        &fixture.internal_base_addr(),
        "/healthz",
        reqwest::StatusCode::OK,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "internal health check failed before auth-like flow test: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let get_response = client
        .get(format!("{}{}", proxied_base_addr, expected_path))
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to send proxied auth-like get request: {error:?}\nlogs:\n{}",
                fixture.logs()
            )
        });
    let get_status = get_response.status();
    let get_headers_debug = format!("{:?}", get_response.headers());
    let get_cookie = get_response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| {
            panic!(
                "expected proxied auth-like get response to set a session cookie, got status {get_status}, headers {get_headers_debug}\nlogs:\n{}",
                fixture.logs()
            )
        });
    let get_body = get_response.text().await.unwrap_or_else(|error| {
        panic!(
            "failed to read proxied auth-like get body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });
    assert!(
        get_body.contains("\"password_fields\":false"),
        "expected identification-stage body on proxied auth-like get, got `{get_body}`\nlogs:\n{}",
        fixture.logs()
    );

    let cookie_value = get_cookie
        .split(';')
        .next()
        .map(str::to_string)
        .unwrap_or_else(|| {
            panic!(
                "expected a cookie-pair prefix in proxied auth-like get set-cookie `{get_cookie}`\nlogs:\n{}",
                fixture.logs()
            )
        });

    let post_response = client
        .post(format!("{}{}", proxied_base_addr, expected_path))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::COOKIE, cookie_value)
        .body(expected_body.to_string())
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to send proxied auth-like post request: {error:?}\nlogs:\n{}",
                fixture.logs()
            )
        });

    let post_status = post_response.status();
    let post_body = post_response.text().await.unwrap_or_else(|error| {
        panic!(
            "failed to read proxied auth-like post body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });
    assert_eq!(
        post_status,
        reqwest::StatusCode::OK,
        "expected auth-like post to succeed, got status {post_status} body `{post_body}`\nlogs:\n{}",
        fixture.logs()
    );
    assert!(
        post_body.contains("\"password_fields\":true"),
        "expected proxied auth-like post to advance the flow, got `{post_body}`\nlogs:\n{}",
        fixture.logs()
    );

    match tokio::time::timeout(tokio::time::Duration::from_secs(20), result_rx).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => {
            panic!(
                "auth-like upstream assertion failed: {error}\nlogs:\n{}",
                fixture.logs()
            )
        }
        Ok(Err(error)) => {
            panic!(
                "auth-like upstream channel closed unexpectedly: {error}\nlogs:\n{}",
                fixture.logs()
            )
        }
        Err(_) => {
            panic!(
                "timed out waiting for auth-like upstream assertion\nlogs:\n{}",
                fixture.logs()
            )
        }
    }
}
