//! HTTP to HTTPS redirect module.
//!
//! Intercepts HTTP requests and issues a 301 redirect to HTTPS equivalent.
//! A request is considered "secure" if:
//! - Scheme is `https`
//! - Port is 443
//! - URI starts with `https://` or `wss://`
//!
//! WebSocket upgrades are always passed through to avoid breaking the
//! HTTP/1.1 upgrade handshake with redirects.

fn is_secure_request(request: &ksbh_modules_sdk::RequestInfo) -> bool {
    let scheme = request.scheme.as_str();
    let uri = request.uri.as_str();

    scheme.eq_ignore_ascii_case("https")
        || request.port == 443
        || uri.starts_with("https://")
        || uri.starts_with("wss://")
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

fn normalized_url_for_compare(input: &str) -> Option<String> {
    let parsed: http::Uri = input.parse().ok()?;
    let scheme = parsed.scheme_str()?;
    let host = parsed.host()?;
    let mut normalized = String::new();
    normalized.push_str(scheme);
    normalized.push_str("://");
    normalized.push_str(host.to_ascii_lowercase().as_str());
    if let Some(port) = parsed.port_u16() {
        let is_default = (scheme == "https" && port == 443)
            || (scheme == "http" && port == 80)
            || (scheme == "wss" && port == 443)
            || (scheme == "ws" && port == 80);
        if !is_default {
            normalized.push(':');
            normalized.push_str(port.to_string().as_str());
        }
    }
    normalized.push_str(parsed.path());
    if let Some(path_and_query) = parsed.path_and_query()
        && let Some(query) = path_and_query.query()
    {
        normalized.push('?');
        normalized.push_str(query);
    }
    Some(normalized)
}

fn is_self_redirect(redirect_url: &str, request_uri: &str) -> bool {
    if redirect_url == request_uri {
        return true;
    }

    match (
        normalized_url_for_compare(redirect_url),
        normalized_url_for_compare(request_uri),
    ) {
        (Some(redirect_norm), Some(request_norm)) => redirect_norm == request_norm,
        _ => false,
    }
}

pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    if ctx.request.is_websocket_handshake {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let secure = is_secure_request(&ctx.request);

    if !secure {
        let redirect_url = build_redirect_url(ctx.request.uri.as_str(), ctx.request.host.as_str());
        if is_self_redirect(&redirect_url, ctx.request.uri.as_str()) {
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }

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
            is_websocket_handshake: false,
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
    fn secure_request_detects_https_scheme_from_request_info() {
        let request = test_request(
            "https://example.com/ws/client/",
            "example.com",
            "https",
            443,
        );

        assert!(super::is_secure_request(&request));
    }

    #[test]
    fn secure_request_detects_wss_uri_from_request_info() {
        let request = test_request("wss://example.com/ws/client/", "example.com", "https", 443);

        assert!(super::is_secure_request(&request));
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

    #[test]
    fn self_redirect_detection_handles_equivalent_https_urls() {
        assert!(super::is_self_redirect(
            "https://charts.ksbh.rs/index.yaml",
            "https://charts.ksbh.rs:443/index.yaml"
        ));
    }
}
