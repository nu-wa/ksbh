mod common;

use futures_util::{SinkExt, StreamExt};

const WEBSOCKET_STEP_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(10);
const WEBSOCKET_ROUTE_READY_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(90);

async fn connect_ws_with_host(
    url: &str,
    host: &str,
) -> tokio_tungstenite::tungstenite::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
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
    .map(|(socket, _response)| socket)
}

async fn connect_wss_with_host(
    url: &str,
    host: &str,
) -> tokio_tungstenite::tungstenite::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
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
    .map(|(socket, _response)| socket)
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
    result: tokio_tungstenite::tungstenite::Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
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

async fn wait_for_ws_connection_ready(
    url: &str,
    host: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let start = tokio::time::Instant::now();
    let mut last_error = ::std::string::String::new();
    while start.elapsed() < WEBSOCKET_ROUTE_READY_TIMEOUT {
        match connect_ws_with_host(url, host).await {
            Ok(result) => return result,
            Err(error) => {
                last_error = error.to_string();
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    panic!(
        "timed out waiting for ws websocket readiness for host {}: {}",
        host, last_error
    );
}

async fn wait_for_wss_connection_ready(
    url: &str,
    host: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let start = tokio::time::Instant::now();
    let mut last_error = ::std::string::String::new();
    while start.elapsed() < WEBSOCKET_ROUTE_READY_TIMEOUT {
        match connect_wss_with_host(url, host).await {
            Ok(result) => return result,
            Err(error) => {
                last_error = error.to_string();
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    panic!(
        "timed out waiting for wss websocket readiness for host {}: {}",
        host, last_error
    );
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

    let ws_socket = connect_ws_with_host(&ws_url, &host)
        .await
        .expect("failed to establish ws websocket through ksbh");
    assert_websocket_roundtrip(ws_socket, &["ping"]).await;

    let wss_socket = connect_wss_with_host(&wss_url, &host)
        .await
        .expect("failed to establish wss websocket through ksbh");
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

    let ws_socket = connect_ws_with_host(&ws_url, &host)
        .await
        .expect("failed to establish ws websocket through ksbh");
    assert_websocket_roundtrip(ws_socket, &["ping-one", "ping-two"]).await;

    let wss_socket = connect_wss_with_host(&wss_url, &host)
        .await
        .expect("failed to establish wss websocket through ksbh");
    assert_websocket_roundtrip(wss_socket, &["ping-one", "ping-two"]).await;

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
}

#[tokio::test]
#[ignore = "requires local kind e2e environment with websocket probe fixture and module support"]
async fn k8s_websocket_ingress_bypasses_http_modules_on_handshake() {
    let config = common::E2eConfig::from_env();
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

    let ws_socket = wait_for_ws_connection_ready(&ws_url, &host).await;
    assert_websocket_roundtrip(ws_socket, &["ping"]).await;

    let wss_socket = wait_for_wss_connection_ready(&wss_url, &host).await;
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
