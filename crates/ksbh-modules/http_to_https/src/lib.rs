//! HTTP to HTTPS redirect module.
//!
//! Intercepts HTTP requests and issues a 301 redirect to HTTPS equivalent.
//! A request is considered "secure" if:
//! - Scheme is `https`
//! - Port is 443
//! - URI starts with `https://` or `wss://`
//! - `x-forwarded-proto` indicates `https`/`wss`
//! - `x-forwarded-port` indicates `443`
//!
//! WebSocket upgrades are always passed through to avoid breaking the
//! HTTP/1.1 upgrade handshake with redirects.

fn header_has_token(
    headers: &http::HeaderMap,
    name: impl http::header::AsHeaderName,
    token: &str,
) -> bool {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(token))
        })
        .unwrap_or(false)
}

fn is_secure_request(request: &ksbh_modules_sdk::RequestInfo, headers: &http::HeaderMap) -> bool {
    let scheme = request.scheme.as_str();
    let uri = request.uri.as_str();

    scheme.eq_ignore_ascii_case("https")
        || request.port == 443
        || uri.starts_with("https://")
        || uri.starts_with("wss://")
        || header_has_token(headers, "x-forwarded-proto", "https")
        || header_has_token(headers, "x-forwarded-proto", "wss")
        || header_has_token(headers, "x-forwarded-port", "443")
}

fn build_redirect_url(uri: &str, host: &str) -> String {
    if uri.starts_with("http://") {
        return uri.replacen("http://", "https://", 1);
    }

    if uri.starts_with("ws://") {
        return uri.replacen("ws://", "wss://", 1);
    }

    if uri.starts_with('/') && !host.is_empty() {
        return format!("https://{}{}", host, uri);
    }

    format!("https://{}", uri)
}

pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    if ksbh_modules_sdk::is_websocket_upgrade_request(&ctx.headers) {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let secure = is_secure_request(&ctx.request, &ctx.headers);

    if !secure {
        let redirect_url = build_redirect_url(ctx.request.uri.as_str(), ctx.request.host.as_str());

        let response = http::Response::builder()
            .status(http::StatusCode::MOVED_PERMANENTLY)
            .header(http::header::LOCATION, redirect_url)
            .body(bytes::Bytes::new())?;

        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::HttpToHttps);

#[cfg(test)]
mod tests {
    fn test_request(
        uri: &str,
        host: &str,
        scheme: &str,
        port: u16,
    ) -> ksbh_modules_sdk::RequestInfo {
        ksbh_modules_sdk::RequestInfo {
            uri: uri.into(),
            host: host.into(),
            method: "GET".into(),
            path: "/".into(),
            query_params: ::std::collections::HashMap::new(),
            scheme: scheme.into(),
            port,
        }
    }

    #[test]
    fn websocket_upgrade_is_detected_with_connection_upgrade() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::UPGRADE,
            http::HeaderValue::from_static("websocket"),
        );
        headers.insert(
            http::header::CONNECTION,
            http::HeaderValue::from_static("keep-alive, Upgrade"),
        );

        assert!(ksbh_modules_sdk::is_websocket_upgrade_request(&headers));
    }

    #[test]
    fn websocket_upgrade_is_detected_with_sec_websocket_key() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::UPGRADE,
            http::HeaderValue::from_static("websocket"),
        );
        headers.insert(
            "Sec-WebSocket-Key",
            http::HeaderValue::from_static("dGhlIHNhbXBsZSBub25jZQ=="),
        );

        assert!(ksbh_modules_sdk::is_websocket_upgrade_request(&headers));
    }

    #[test]
    fn websocket_upgrade_requires_upgrade_websocket_header() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::CONNECTION,
            http::HeaderValue::from_static("upgrade"),
        );
        headers.insert(
            "Sec-WebSocket-Key",
            http::HeaderValue::from_static("dGhlIHNhbXBsZSBub25jZQ=="),
        );

        assert!(!ksbh_modules_sdk::is_websocket_upgrade_request(&headers));
    }

    #[test]
    fn secure_request_detects_forwarded_https() {
        let request = test_request("http://example.com/ws/client/", "example.com", "http", 80);
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-proto", http::HeaderValue::from_static("https"));

        assert!(super::is_secure_request(&request, &headers));
    }

    #[test]
    fn secure_request_detects_forwarded_wss() {
        let request = test_request("ws://example.com/ws/client/", "example.com", "http", 80);
        let mut headers = http::HeaderMap::new();
        headers.insert("x-forwarded-proto", http::HeaderValue::from_static("wss"));

        assert!(super::is_secure_request(&request, &headers));
    }

    #[test]
    fn redirect_url_for_relative_path_uses_host() {
        let redirect = super::build_redirect_url("/ws/client/", "authentik.yannis.codes");
        assert_eq!(redirect, "https://authentik.yannis.codes/ws/client/");
    }

    #[test]
    fn redirect_url_upgrades_ws_to_wss() {
        let redirect = super::build_redirect_url("ws://authentik.yannis.codes/ws/client/", "");
        assert_eq!(redirect, "wss://authentik.yannis.codes/ws/client/");
    }
}
