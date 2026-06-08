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
    _stage: ksbh_modules_sdk::RequestStage,
    ctx: ksbh_modules_sdk::ModuleContext,
) -> ksbh_modules_sdk::RequestResult {
    if ctx.request_info.is_websocket_handshake {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let secure = ctx.request_info.scheme.eq_ignore_ascii_case("https")
        || ctx.request_info.port == 443
        || ctx.request_info.uri.starts_with("https://")
        || ctx.request_info.uri.starts_with("wss://");

    if !secure {
        let redirect_url = build_redirect_url(ctx.request_info.uri, ctx.request_info.host);
        if is_self_redirect(&redirect_url, ctx.request_info.uri) {
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }

        let response = http::Response::builder()
            .status(http::StatusCode::MOVED_PERMANENTLY)
            .header(http::header::LOCATION, redirect_url)
            .body(bytes::Bytes::new())?;

        return Ok(ksbh_modules_sdk::ModuleResult::Stop(Some(response)));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

#[cfg(test)]
mod tests {
    use super::*;

    // build_redirect_url tests
    #[test]
    fn redirect_http_to_https() {
        assert_eq!(
            build_redirect_url("http://example.com/path", "example.com"),
            "https://example.com/path"
        );
    }

    #[test]
    fn redirect_ws_to_wss() {
        assert_eq!(
            build_redirect_url("ws://example.com/ws", "example.com"),
            "wss://example.com/ws"
        );
    }

    #[test]
    fn redirect_absolute_path_with_host() {
        assert_eq!(
            build_redirect_url("/path?query=1", "example.com"),
            "https://example.com/path?query=1"
        );
    }

    #[test]
    fn redirect_defaults_to_https_prefix() {
        assert_eq!(
            build_redirect_url("example.com", ""),
            "https://example.com"
        );
    }

    // normalized_url_for_compare tests
    #[test]
    fn normalizes_lowercase_host() {
        let a = normalized_url_for_compare("https://EXAMPLE.COM/path").unwrap();
        let b = normalized_url_for_compare("https://example.com/path").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn normalizes_default_http_port() {
        let a = normalized_url_for_compare("http://example.com:80/path").unwrap();
        let b = normalized_url_for_compare("http://example.com/path").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn normalizes_default_https_port() {
        let a = normalized_url_for_compare("https://example.com:443/path").unwrap();
        let b = normalized_url_for_compare("https://example.com/path").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn preserves_non_default_port() {
        let result = normalized_url_for_compare("https://example.com:8443/path").unwrap();
        assert!(result.contains(":8443"));
    }

    #[test]
    fn includes_query_string() {
        let result = normalized_url_for_compare("https://example.com/path?key=value").unwrap();
        assert!(result.contains("?key=value"));
    }

    #[test]
    fn invalid_url_returns_none() {
        assert!(normalized_url_for_compare("not a url").is_none());
    }

    // is_self_redirect tests
    #[test]
    fn detects_identical_urls() {
        assert!(is_self_redirect(
            "https://example.com/path",
            "https://example.com/path"
        ));
    }

    #[test]
    fn detects_normalized_match() {
        assert!(is_self_redirect(
            "https://EXAMPLE.COM/path",
            "https://example.com/path"
        ));
    }

    #[test]
    fn different_urls_are_not_self_redirect() {
        assert!(!is_self_redirect(
            "https://example.com/other",
            "https://example.com/path"
        ));
    }

    #[test]
    fn http_to_https_is_not_self_redirect() {
        assert!(!is_self_redirect(
            "https://example.com/path",
            "http://example.com/path"
        ));
    }
}

ksbh_modules_sdk::export_module!(
    process,
    ksbh_modules_sdk::module_definition!(
        ksbh_modules_sdk::abi::prelude::KSBHModuleKind::HttpToHttps,
        [ksbh_modules_sdk::RequestStage::BeforeRouting, ksbh_modules_sdk::RequestStage::Request]
    )
);
