mod common;

use futures_util::{SinkExt, StreamExt};

const WEBSOCKET_STEP_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(10);
const WEBSOCKET_ROUTE_READY_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(90);

async fn connect_ws_with_host(
    url: &str,
    host: &str,
) -> tokio_tungstenite::tungstenite::Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Option<::std::string::String>,
)> {
    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url)?;
    request.headers_mut().insert(
        http::header::HOST,
        http::HeaderValue::from_str(host)
            .expect("failed to convert websocket host header to header value"),
    );

    tokio::time::timeout(
        WEBSOCKET_STEP_TIMEOUT,
        tokio_tungstenite::connect_async(request),
    )
    .await
    .expect("timed out establishing ws websocket through ksbh")
    .map(|(socket, response)| {
        let transport = response
            .headers()
            .get(ksbh_core::constants::HEADER_X_KSBH_WS_DOWNSTREAM_TRANSPORT)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        (socket, transport)
    })
}

async fn connect_wss_with_host(
    url: &str,
    host: &str,
) -> tokio_tungstenite::tungstenite::Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Option<::std::string::String>,
)> {
    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url)?;
    request.headers_mut().insert(
        http::header::HOST,
        http::HeaderValue::from_str(host)
            .expect("failed to convert secure websocket host header to header value"),
    );

    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("failed to build permissive TLS connector for e2e");
    let connector = tokio_tungstenite::Connector::NativeTls(tls);

    tokio::time::timeout(
        WEBSOCKET_STEP_TIMEOUT,
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector)),
    )
    .await
    .expect("timed out establishing wss websocket through ksbh")
    .map(|(socket, response)| {
        let transport = response
            .headers()
            .get(ksbh_core::constants::HEADER_X_KSBH_WS_DOWNSTREAM_TRANSPORT)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        (socket, transport)
    })
}

fn build_masked_text_websocket_frame(payload: &str) -> bytes::Bytes {
    let payload = payload.as_bytes();
    let mut frame = Vec::with_capacity(2 + 4 + payload.len());
    frame.push(0x81);
    frame.push(0x80 | (payload.len() as u8));
    let mask_key = [0x11, 0x22, 0x33, 0x44];
    frame.extend_from_slice(&mask_key);
    for (index, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask_key[index % 4]);
    }
    bytes::Bytes::from(frame)
}

fn build_masked_close_websocket_frame() -> bytes::Bytes {
    let mut frame = Vec::with_capacity(6);
    frame.push(0x88);
    frame.push(0x80);
    frame.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);
    bytes::Bytes::from(frame)
}

fn try_parse_unmasked_text_websocket_frame(buffer: &mut Vec<u8>) -> Option<::std::string::String> {
    if buffer.len() < 2 {
        return None;
    }

    let first = buffer[0];
    let second = buffer[1];
    let opcode = first & 0x0F;
    let masked = (second & 0x80) != 0;
    let mut payload_len = (second & 0x7F) as usize;
    let mut offset = 2usize;

    if payload_len == 126 {
        if buffer.len() < offset + 2 {
            return None;
        }
        payload_len = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]) as usize;
        offset += 2;
    } else if payload_len == 127 {
        if buffer.len() < offset + 8 {
            return None;
        }
        payload_len = u64::from_be_bytes([
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]) as usize;
        offset += 8;
    }

    if masked {
        if buffer.len() < offset + 4 {
            return None;
        }
        offset += 4;
    }

    if buffer.len() < offset + payload_len {
        return None;
    }

    let payload = buffer[offset..offset + payload_len].to_vec();
    buffer.drain(0..offset + payload_len);

    if opcode == 0x8 {
        return Some("__close__".to_string());
    }

    if opcode != 0x1 {
        return Some(format!("__opcode:{}__", opcode));
    }

    Some(
        ::std::string::String::from_utf8(payload)
            .expect("failed to parse websocket text frame payload as utf-8"),
    )
}

async fn read_next_h2_websocket_text_frame(
    recv_stream: &mut h2::RecvStream,
    frame_buffer: &mut Vec<u8>,
) -> ::std::string::String {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);

    loop {
        if let Some(parsed) = try_parse_unmasked_text_websocket_frame(frame_buffer) {
            return parsed;
        }

        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .unwrap_or_else(|| tokio::time::Duration::from_secs(0));
        let next_data = tokio::time::timeout(remaining, recv_stream.data())
            .await
            .expect("timed out waiting for h2 websocket data");

        match next_data {
            Some(Ok(chunk)) => frame_buffer.extend_from_slice(chunk.as_ref()),
            Some(Err(error)) => panic!("failed to read h2 websocket data frame: {error}"),
            None => panic!("h2 websocket stream closed before receiving expected frame"),
        }
    }
}

async fn connect_wss_h2_extended_with_host(
    url: &str,
    host: &str,
) -> (
    h2::SendStream<bytes::Bytes>,
    h2::RecvStream,
    Option<::std::string::String>,
) {
    let parsed_url = reqwest::Url::parse(url).expect("failed to parse wss url for h2 websocket");
    let tcp_host = parsed_url
        .host_str()
        .expect("missing host in wss url for h2 websocket");
    let tcp_port = parsed_url
        .port_or_known_default()
        .expect("missing port in wss url for h2 websocket");
    let path = parsed_url.path();
    let path_and_query = match parsed_url.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    };

    let tcp = tokio::net::TcpStream::connect(format!("{tcp_host}:{tcp_port}"))
        .await
        .expect("failed to open tcp socket for h2 websocket");
    let mut tls_builder = native_tls::TlsConnector::builder();
    tls_builder.danger_accept_invalid_certs(true);
    tls_builder.request_alpns(&["h2"]);
    let tls_connector = tokio_native_tls::TlsConnector::from(
        tls_builder
            .build()
            .expect("failed to build native tls connector for h2 websocket"),
    );
    let tls = tls_connector
        .connect(tcp_host, tcp)
        .await
        .expect("failed to complete tls handshake for h2 websocket");

    let negotiated_alpn = tls
        .get_ref()
        .negotiated_alpn()
        .expect("failed to inspect negotiated ALPN");
    assert_eq!(
        negotiated_alpn.as_deref(),
        Some(b"h2".as_slice()),
        "expected ALPN to negotiate h2 for extended CONNECT websocket",
    );

    let h2_builder = h2::client::Builder::new();
    let (mut h2_client, h2_connection) = h2_builder
        .handshake(tls)
        .await
        .expect("failed to establish h2 client session");
    tokio::spawn(async move {
        if let Err(error) = h2_connection.await {
            panic!("h2 client connection failed: {error}");
        }
    });

    let settings_deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    while !h2_client.is_extended_connect_protocol_enabled() {
        if tokio::time::Instant::now() > settings_deadline {
            panic!("h2 peer did not advertise SETTINGS_ENABLE_CONNECT_PROTOCOL");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let mut request = http::Request::builder()
        .version(http::Version::HTTP_2)
        .method(http::Method::CONNECT)
        .uri(format!("https://{host}{path_and_query}"))
        .header(http::header::HOST, host)
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("sec-websocket-version", "13")
        .body(())
        .expect("failed to build h2 extended CONNECT websocket request");
    request
        .extensions_mut()
        .insert(h2::ext::Protocol::from_static("websocket"));

    let (response_future, send_stream) = h2_client
        .send_request(request, false)
        .expect("failed to send h2 extended CONNECT websocket request");
    let response = response_future
        .await
        .expect("failed to receive h2 extended CONNECT websocket response");
    assert_eq!(
        response.status(),
        http::StatusCode::OK,
        "expected successful h2 extended CONNECT websocket status",
    );

    let transport = response
        .headers()
        .get(ksbh_core::constants::HEADER_X_KSBH_WS_DOWNSTREAM_TRANSPORT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    (send_stream, response.into_body(), transport)
}

async fn assert_websocket_roundtrip(
    mut socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected_messages: &[&str],
) {
    let ready = tokio::time::timeout(WEBSOCKET_STEP_TIMEOUT, socket.next())
        .await
        .expect("timed out waiting for websocket readiness frame")
        .expect("websocket stream ended before probe ready message")
        .expect("failed to read probe ready frame");

    match ready {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            assert_eq!(text, "ready", "unexpected websocket readiness frame");
        }
        other => {
            panic!("expected websocket text readiness frame, got {other:?}");
        }
    }

    for expected_message in expected_messages {
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                (*expected_message).to_string(),
            ))
            .await
            .expect("failed to send websocket frame through ksbh");

        let echoed = tokio::time::timeout(WEBSOCKET_STEP_TIMEOUT, socket.next())
            .await
            .expect("timed out waiting for websocket echo frame")
            .expect("websocket stream ended before echo frame")
            .expect("failed to read websocket echo frame");

        match echoed {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                assert_eq!(
                    text,
                    format!("echo:{expected_message}"),
                    "unexpected websocket echo payload"
                );
            }
            other => {
                panic!("expected websocket text echo frame, got {other:?}");
            }
        }
    }

    socket
        .close(None)
        .await
        .expect("failed to close websocket connection");

    let close_frame = tokio::time::timeout(tokio::time::Duration::from_secs(5), socket.next())
        .await
        .expect("timed out waiting for websocket close completion");

    match close_frame {
        Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => {}
        Some(Ok(other)) => {
            panic!("expected websocket close frame or EOF after close, got {other:?}");
        }
        Some(Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed)) => {}
        Some(Err(error)) => {
            panic!("failed to read websocket close completion: {error}");
        }
    }
}

async fn assert_websocket_connection_rejected_with_status(
    result: tokio_tungstenite::tungstenite::Result<(
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Option<::std::string::String>,
    )>,
    expected_status: reqwest::StatusCode,
) {
    let error = result.expect_err("expected websocket handshake to fail");

    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(
                response.status(),
                expected_status,
                "unexpected websocket handshake status"
            );
        }
        other => {
            panic!(
                "expected websocket handshake to fail with HTTP {}, got {other:?}",
                expected_status
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires local kind e2e environment with websocket probe fixture"]
async fn k8s_websocket_ingress_supports_wss_h2_extended_connect_roundtrip() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let ingress_name = common::unique_name("websocket-ingress");
    let host = common::unique_host("websocket");

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-websocket-probe",
        &[],
        &[],
    )
    .await;

    let start = tokio::time::Instant::now();
    let timeout = WEBSOCKET_ROUTE_READY_TIMEOUT;
    let mut last_status = reqwest::StatusCode::NOT_FOUND;
    while start.elapsed() < timeout {
        match common::get_with_host(&client, &config.http_addr, "/ws", &host).await {
            Ok(response) => {
                let status = response.status();
                last_status = status;
                if status != reqwest::StatusCode::NOT_FOUND {
                    break;
                }
            }
            Err(_) => {}
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    assert_ne!(
        last_status,
        reqwest::StatusCode::NOT_FOUND,
        "websocket ingress route was not available before websocket dial",
    );

    let wss_url = format!(
        "{}/ws",
        config
            .https_addr
            .replace("https://", "wss://")
            .trim_end_matches('/')
    );

    let (mut send_stream, mut recv_stream, transport) =
        connect_wss_h2_extended_with_host(&wss_url, &host).await;
    assert_eq!(
        transport.as_deref(),
        Some("h2"),
        "expected downstream websocket transport to be h2 for extended CONNECT",
    );

    let mut frame_buffer = Vec::new();
    let ready = read_next_h2_websocket_text_frame(&mut recv_stream, &mut frame_buffer).await;
    assert_eq!(ready, "ready", "unexpected websocket readiness frame");

    send_stream
        .send_data(build_masked_text_websocket_frame("ping"), false)
        .expect("failed to send masked websocket text frame over h2 stream");
    let echoed = read_next_h2_websocket_text_frame(&mut recv_stream, &mut frame_buffer).await;
    assert_eq!(echoed, "echo:ping", "unexpected websocket echo payload");

    send_stream
        .send_data(build_masked_close_websocket_frame(), false)
        .expect("failed to send masked websocket close frame over h2 stream");
    send_stream
        .send_data(bytes::Bytes::new(), true)
        .expect("failed to finish h2 websocket stream");

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
}

#[tokio::test]
#[ignore = "requires local kind e2e environment with websocket probe fixture"]
async fn k8s_websocket_ingress_supports_ws_and_wss_roundtrip() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let ingress_name = common::unique_name("websocket-ingress");
    let host = common::unique_host("websocket");

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-websocket-probe",
        &[],
        &[],
    )
    .await;

    // Wait for ingress reconciliation/route propagation before websocket dial attempts.
    let start = tokio::time::Instant::now();
    let timeout = WEBSOCKET_ROUTE_READY_TIMEOUT;
    let mut last_status = reqwest::StatusCode::NOT_FOUND;
    while start.elapsed() < timeout {
        match common::get_with_host(&client, &config.http_addr, "/ws", &host).await {
            Ok(response) => {
                let status = response.status();
                last_status = status;
                if status != reqwest::StatusCode::NOT_FOUND {
                    break;
                }
            }
            Err(_) => {}
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    assert_ne!(
        last_status,
        reqwest::StatusCode::NOT_FOUND,
        "websocket ingress route was not available before websocket dial",
    );

    let ws_url = format!(
        "{}/ws",
        config
            .http_addr
            .replace("http://", "ws://")
            .trim_end_matches('/')
    );
    let wss_url = format!(
        "{}/ws",
        config
            .https_addr
            .replace("https://", "wss://")
            .trim_end_matches('/')
    );

    let (ws_socket, ws_transport) = connect_ws_with_host(&ws_url, &host)
        .await
        .expect("failed to establish ws websocket through ksbh");
    assert!(
        matches!(ws_transport.as_deref(), None | Some("h1")),
        "expected ws downstream transport header to be absent or h1, got {:?}",
        ws_transport
    );
    assert_websocket_roundtrip(ws_socket, &["ping"]).await;

    let (wss_socket, wss_transport) = connect_wss_with_host(&wss_url, &host)
        .await
        .expect("failed to establish wss websocket through ksbh");
    assert!(
        matches!(wss_transport.as_deref(), None | Some("h1") | Some("h2")),
        "expected downstream transport header to be absent/h1/h2, got {:?}",
        wss_transport
    );
    assert_websocket_roundtrip(wss_socket, &["ping"]).await;

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
}

#[tokio::test]
#[ignore = "requires local kind e2e environment with websocket probe fixture"]
async fn k8s_websocket_ingress_supports_multi_message_echo_and_close() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let ingress_name = common::unique_name("websocket-ingress");
    let host = common::unique_host("websocket");

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-websocket-probe",
        &[],
        &[],
    )
    .await;

    let start = tokio::time::Instant::now();
    let timeout = WEBSOCKET_ROUTE_READY_TIMEOUT;
    let mut last_status = reqwest::StatusCode::NOT_FOUND;
    while start.elapsed() < timeout {
        match common::get_with_host(&client, &config.http_addr, "/ws", &host).await {
            Ok(response) => {
                let status = response.status();
                last_status = status;
                if status != reqwest::StatusCode::NOT_FOUND {
                    break;
                }
            }
            Err(_) => {}
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    assert_ne!(
        last_status,
        reqwest::StatusCode::NOT_FOUND,
        "websocket ingress route was not available before websocket dial",
    );

    let ws_url = format!(
        "{}/ws",
        config
            .http_addr
            .replace("http://", "ws://")
            .trim_end_matches('/')
    );
    let wss_url = format!(
        "{}/ws",
        config
            .https_addr
            .replace("https://", "wss://")
            .trim_end_matches('/')
    );

    let (ws_socket, _) = connect_ws_with_host(&ws_url, &host)
        .await
        .expect("failed to establish ws websocket through ksbh");
    assert_websocket_roundtrip(ws_socket, &["ping-one", "ping-two"]).await;

    let (wss_socket, _) = connect_wss_with_host(&wss_url, &host)
        .await
        .expect("failed to establish wss websocket through ksbh");
    assert_websocket_roundtrip(wss_socket, &["ping-one", "ping-two"]).await;

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
}

#[tokio::test]
#[ignore = "requires local kind e2e environment with websocket probe fixture and module support"]
async fn k8s_websocket_ingress_bypasses_http_modules_on_handshake() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let ingress_name = common::unique_name("websocket-ingress");
    let host = common::unique_host("websocket");
    let module_name = common::unique_name("pow");
    let secret_name = common::unique_name("pow-secret");

    common::create_pow_module(
        &kube_client,
        &config.namespace,
        &module_name,
        &secret_name,
        "websocket-bypass-secret",
        1,
    )
    .await;

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-websocket-probe",
        &[module_name.as_str()],
        &[],
    )
    .await;

    let start = tokio::time::Instant::now();
    let timeout = WEBSOCKET_ROUTE_READY_TIMEOUT;
    let mut last_status = reqwest::StatusCode::NOT_FOUND;
    while start.elapsed() < timeout {
        match common::get_with_host(&client, &config.http_addr, "/ws", &host).await {
            Ok(response) => {
                let status = response.status();
                last_status = status;
                if status != reqwest::StatusCode::NOT_FOUND {
                    break;
                }
            }
            Err(_) => {}
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    assert_ne!(
        last_status,
        reqwest::StatusCode::NOT_FOUND,
        "websocket ingress route was not available before websocket dial",
    );

    let ws_url = format!(
        "{}/ws",
        config
            .http_addr
            .replace("http://", "ws://")
            .trim_end_matches('/')
    );
    let wss_url = format!(
        "{}/ws",
        config
            .https_addr
            .replace("https://", "wss://")
            .trim_end_matches('/')
    );

    let (ws_socket, ws_transport) = connect_ws_with_host(&ws_url, &host)
        .await
        .expect("failed to establish ws websocket through ksbh");
    assert!(
        matches!(ws_transport.as_deref(), None | Some("h1")),
        "expected ws downstream transport header to be absent or h1, got {:?}",
        ws_transport
    );
    assert_websocket_roundtrip(ws_socket, &["ping"]).await;

    let (wss_socket, wss_transport) = connect_wss_with_host(&wss_url, &host)
        .await
        .expect("failed to establish wss websocket through ksbh");
    assert!(
        matches!(wss_transport.as_deref(), None | Some("h1") | Some("h2")),
        "expected downstream transport header to be absent/h1/h2, got {:?}",
        wss_transport
    );
    assert_websocket_roundtrip(wss_socket, &["ping"]).await;

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
    common::delete_module_configuration(&kube_client, &module_name).await;
    common::delete_secret(&kube_client, &config.namespace, &secret_name).await;
}

#[tokio::test]
#[ignore = "requires local kind e2e environment with websocket probe fixture"]
async fn k8s_websocket_ingress_rejects_unknown_host_for_websocket_upgrade() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let ingress_name = common::unique_name("websocket-ingress");
    let host = common::unique_host("websocket");
    let missing_host = common::unique_host("websocket-missing");

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-websocket-probe",
        &[],
        &[],
    )
    .await;

    let start = tokio::time::Instant::now();
    let timeout = WEBSOCKET_ROUTE_READY_TIMEOUT;
    let mut last_status = reqwest::StatusCode::NOT_FOUND;
    while start.elapsed() < timeout {
        match common::get_with_host(&client, &config.http_addr, "/ws", &host).await {
            Ok(response) => {
                let status = response.status();
                last_status = status;
                if status != reqwest::StatusCode::NOT_FOUND {
                    break;
                }
            }
            Err(_) => {}
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    assert_ne!(
        last_status,
        reqwest::StatusCode::NOT_FOUND,
        "websocket ingress route was not available before websocket dial",
    );

    let ws_url = format!(
        "{}/ws",
        config
            .http_addr
            .replace("http://", "ws://")
            .trim_end_matches('/')
    );
    let wss_url = format!(
        "{}/ws",
        config
            .https_addr
            .replace("https://", "wss://")
            .trim_end_matches('/')
    );

    assert_websocket_connection_rejected_with_status(
        connect_ws_with_host(&ws_url, &missing_host).await,
        reqwest::StatusCode::NOT_FOUND,
    )
    .await;
    assert_websocket_connection_rejected_with_status(
        connect_wss_with_host(&wss_url, &missing_host).await,
        reqwest::StatusCode::NOT_FOUND,
    )
    .await;

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
}
