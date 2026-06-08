pub(super) mod trait_methods;

/// The main proxy service implementation combining routing, modules, storage, and metrics.
///
/// Orchestrates the request lifecycle: request filtering, upstream peer resolution,
/// header manipulation, response filtering, and metrics collection.
#[allow(dead_code)]
pub struct ProxyService {
    #[allow(dead_code)]
    pub(super) storage: ::std::sync::Arc<crate::Storage>,
    #[allow(dead_code)]
    pub(super) sessions: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    #[allow(dead_code)]
    pub(super) config: ::std::sync::Arc<crate::Config>,
    #[allow(dead_code)]
    pub(super) hosts: crate::routing::RouterReader,
    #[allow(dead_code)]
    pub(super) metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
    #[allow(dead_code)]
    pub(super) modules: ::std::sync::Arc<crate::modules::runtime::module_host::ModuleHost>,
    pub(super) cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    pub(super) proxy_header_name: http::header::HeaderName,
    pub(super) proxy_header_value: http::HeaderValue,
}

impl ProxyService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: ::std::sync::Arc<crate::Config>,
        storage: ::std::sync::Arc<crate::Storage>,
        hosts: crate::routing::RouterReader,
        metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
        sessions: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
        modules: ::std::sync::Arc<crate::modules::runtime::module_host::ModuleHost>,
        cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    ) -> Self {
        let proxy_header_name =
            http::header::HeaderName::from_bytes(config.constants.proxy_header_name.as_bytes())
                .expect("validated proxy header name must parse");
        let proxy_header_value = http::HeaderValue::from_str(&config.constants.proxy_header_value)
            .expect("validated proxy header value must parse");

        Self {
            modules,
            storage,
            sessions,
            config: config.clone(),
            hosts,
            metrics_sender,
            cookie_settings,
            proxy_header_name,
            proxy_header_value,
        }
    }

    fn has_explicitly_empty_body(headers: &http::HeaderMap) -> bool {
        headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim() == "0")
            .unwrap_or(false)
    }

    fn render_error_page_html(status_code: u16) -> Option<bytes::Bytes> {
        ksbh_ui::error_pages::render_error_page_html(&status_code.to_string())
            .map(bytes::Bytes::from)
    }

    /// Composes the final value for a forwarded header based on proxy trust.
    ///
    /// When the incoming connection is from a **trusted proxy**, the existing
    /// header value is preserved and this proxy's value is appended with `", "`.
    /// This maintains the full proxy chain.
    ///
    /// When the incoming connection is from an **untrusted source**, the
    /// existing header value is discarded entirely (to prevent IP spoofing)
    /// and only this proxy's value is returned.
    fn compose_forwarded_header_value(
        existing_value: Option<&http::header::HeaderValue>,
        appended_value: &str,
        trust_forwarded_headers: bool,
    ) -> String {
        if !trust_forwarded_headers {
            return appended_value.to_string();
        }

        let Some(existing_value) = existing_value else {
            return appended_value.to_string();
        };

        let Ok(existing_value) = existing_value.to_str() else {
            return appended_value.to_string();
        };
        let existing_value = existing_value.trim();
        if existing_value.is_empty() {
            return appended_value.to_string();
        }

        format!("{existing_value}, {appended_value}")
    }

    fn format_forwarded_for_value(ip: &::std::net::IpAddr) -> String {
        match ip {
            ::std::net::IpAddr::V4(v4) => v4.to_string(),
            ::std::net::IpAddr::V6(v6) => format!("\"[{v6}]\""),
        }
    }

    fn escape_forwarded_value(raw: &str) -> String {
        let mut escaped = String::with_capacity(raw.len());
        for char in raw.chars() {
            match char {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                _ => escaped.push(char),
            }
        }

        escaped
    }

    fn forwarded_header_entry(
        client_ip: Option<::std::net::IpAddr>,
        proto: &str,
        host: &str,
    ) -> String {
        let mut parts = Vec::with_capacity(3);

        match client_ip {
            Some(ip) => parts.push(format!("for={}", Self::format_forwarded_for_value(&ip))),
            None => parts.push("for=unknown".to_string()),
        };

        parts.push(format!("proto=\"{}\"", Self::escape_forwarded_value(proto)));
        parts.push(format!("host=\"{}\"", Self::escape_forwarded_value(host)));

        parts.join(";")
    }

    fn normalize_cookie_header_for_upstream(
        upstream_request: &mut pingora_http::RequestHeader,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        let merged_cookie = upstream_request
            .headers
            .get_all(http::header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<&str>>()
            .join("; ");

        if merged_cookie.is_empty() {
            return Ok(());
        }

        upstream_request.remove_header(&http::header::COOKIE);
        upstream_request
            .insert_header(
                http::header::COOKIE,
                http::HeaderValue::from_str(merged_cookie.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        Ok(())
    }

    async fn build_observed_request(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<crate::proxy::ObservedRequest, ObservedRequestBuildError> {
        let client_information =
            crate::proxy::ClientInformation::new_from_session(session, &self.config)
                .ok_or(ObservedRequestBuildError::MissingClientInformation)?;

        if ctx.parsed_cookie.is_none() {
            ctx.parsed_cookie =
                crate::cookies::ProxyCookie::from_session(&self.cookie_settings, session)
                    .await
                    .ok();
        }

        ctx.needs_session_cookie = ctx.parsed_cookie.is_none();

        let session_id = ctx
            .parsed_cookie
            .as_ref()
            .map(|cookie| cookie.session_id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        let req_id = ctx.req_id;
        let headers = session.headers();
        let trust_forwarded_headers = self
            .config
            .trusts_forwarded_headers_from(session.client_addr());
        let downstream_tls = session
            .server_addr()
            .map(|addr| addr.port() == self.config.listen_addresses.https.port())
            .unwrap_or(false);

        let http_request = ksbh_types::requests::http_request::HttpRequest::new(
            &headers,
            req_id,
            &self.config.ports.external,
            downstream_tls,
            trust_forwarded_headers,
        )
        .map_err(|_| ObservedRequestBuildError::InvalidHttpRequest)?;

        ctx.session_id_bytes = session_id.into_bytes();

        Ok(crate::proxy::ObservedRequest {
            req_id,
            started_at: ctx.req_start,
            client: client_information,
            session_id,
            http_request,
            is_websocket_handshake: ctx.downstream_ws_kind
                != crate::proxy::DownstreamWebsocket::None,
        })
    }
}

#[derive(Debug)]
enum ObservedRequestBuildError {
    MissingClientInformation,
    InvalidHttpRequest,
}

impl ObservedRequestBuildError {
    fn response(&self) -> (http::StatusCode, bytes::Bytes) {
        match self {
            Self::MissingClientInformation => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::from_static(b"Bad Request"),
            ),
            Self::InvalidHttpRequest => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                bytes::Bytes::from_static(b"Internal Server Error"),
            ),
        }
    }
}

impl ProxyService {
    pub(crate) fn new_context(&self) -> crate::proxy::ProxyContext {
        let mut ctx = crate::proxy::ProxyContext::new(self.config.clone());
        crate::metrics::runtime_signals::RUNTIME_SIGNALS.request_started();
        ctx.needs_completion_signal = true;
        ctx
    }

    pub(crate) async fn early_request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        Ok(self.run_early_modules(session, ctx).await?)
    }

    pub(crate) async fn request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        Ok(self.run_request_modules(session, ctx).await?)
    }

    pub(crate) async fn upstream_peer(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<ksbh_types::providers::proxy::UpstreamPeer, ksbh_types::prelude::ProxyProviderError>
    {
        self.resolve_upstream_peer(session, ctx).await
    }

    pub(crate) async fn response_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        response: &mut http::response::Parts,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        self.apply_response_filter(session, response, ctx)
            .await
    }

    pub(crate) fn response_body_filter(
        &self,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if body.as_ref().is_some_and(|chunk| !chunk.is_empty()) {
            ctx.upstream_response_body_seen = true;
        }

        if end_of_stream
            && !ctx.upstream_response_body_seen
            && body.as_ref().is_none_or(bytes::Bytes::is_empty)
            && let Some(fallback_body) = ctx.fallback_error_page_body.take()
        {
            *body = Some(fallback_body);
            ctx.upstream_response_body_seen = true;
        }

        Ok(())
    }

    pub(crate) async fn upstream_request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        upstream_request: &mut pingora_http::RequestHeader,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        self.upstream_request_filter_impl(session, upstream_request, ctx)
            .await
    }

    pub(crate) async fn logging(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        _error: Option<&ksbh_types::prelude::ProxyProviderError>,
        ctx: &mut crate::proxy::ProxyContext,
    ) {
        self.logging_impl(session, _error, ctx).await
    }

    pub(crate) async fn fail_to_proxy(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        error_code: u16,
        _ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<bool, ksbh_types::prelude::ProxyProviderError> {
        if !(400..=599).contains(&error_code) {
            return Ok(false);
        }

        let Some(body) = Self::render_error_page_html(error_code) else {
            return Ok(false);
        };
        let status = http::StatusCode::from_u16(error_code)
            .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
        let response = http::Response::builder()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(body)
            .map_err(|e| {
                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
            })?;

        session.write_response(response).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalize_cookie_header_for_upstream_merges_multiple_headers() {
        let mut request =
            pingora_http::RequestHeader::build_no_case(http::Method::GET, b"/", Some(4))
                .expect("request build should succeed");
        request
            .append_header(http::header::COOKIE, "ksbh=abc")
            .expect("append cookie should succeed");
        request
            .append_header(http::header::COOKIE, "authentik_session=def")
            .expect("append cookie should succeed");

        super::ProxyService::normalize_cookie_header_for_upstream(&mut request)
            .expect("normalization should succeed");

        let cookies: Vec<&http::HeaderValue> = request
            .headers
            .get_all(http::header::COOKIE)
            .iter()
            .collect();
        assert_eq!(cookies.len(), 1);
        assert_eq!(
            cookies[0].to_str().expect("cookie must be utf-8"),
            "ksbh=abc; authentik_session=def"
        );
    }

    #[test]
    fn compose_forwarded_header_value_ignores_untrusted_existing_value() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            crate::constants::HEADER_X_FORWARDED_FOR,
            http::HeaderValue::from_static("198.51.100.9"),
        );

        let appended = super::ProxyService::compose_forwarded_header_value(
            headers.get(crate::constants::HEADER_X_FORWARDED_FOR),
            "203.0.113.8",
            false,
        );

        assert_eq!(appended, "203.0.113.8");
    }

    #[test]
    fn compose_forwarded_header_value_appends_for_trusted_proxy() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            crate::constants::HEADER_X_FORWARDED_FOR,
            http::HeaderValue::from_static("198.51.100.9"),
        );

        let appended = super::ProxyService::compose_forwarded_header_value(
            headers.get(crate::constants::HEADER_X_FORWARDED_FOR),
            "203.0.113.8",
            true,
        );

        assert_eq!(appended, "198.51.100.9, 203.0.113.8");
    }

    #[test]
    fn forwarded_entry_formats_ipv6_and_quotes_host_and_proto() {
        let entry = super::ProxyService::forwarded_header_entry(
            Some("2001:db8::1".parse().expect("parse IPv6 address")),
            "https",
            "example.test:443",
        );

        assert_eq!(
            entry,
            "for=\"[2001:db8::1]\";proto=\"https\";host=\"example.test:443\""
        );
    }
}
