use blake3::Hasher as Blake3Hasher;

pub mod pingora_bridge;
pub mod service;
pub use service::ProxyService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownstreamWebsocket {
    None,
    H1Upgrade,
}

/// Context passed through the entire proxy request lifecycle.
///
/// Contains configuration, parsed request data, module metrics, and session state.
#[derive(Debug)]
pub struct ProxyContext {
    pub config: ::std::sync::Arc<crate::config::Config>,
    pub modules_metrics: Vec<crate::metrics::module_metric::ModuleMetric>,
    pub valid_request_information: Option<ValidRequestInformation>,
    pub observed_request: Option<ObservedRequest>,
    pub routed_request: Option<RoutedRequest>,
    pub req_start: ::std::time::Instant,
    pub req_id: uuid::Uuid,
    pub proxy_decision: Option<ksbh_types::prelude::ProxyDecision>,
    pub parsed_cookie: Option<crate::cookies::ProxyCookie>,
    pub needs_session_cookie: bool,
    pub http_request: Option<ksbh_types::requests::http_request::HttpRequest>,
    pub downstream_ws_kind: DownstreamWebsocket,
    pub session_id_bytes: [u8; 16],
    pub buffered_request_body: Option<bytes::Bytes>,
    pub fallback_error_page_body: Option<bytes::Bytes>,
    pub upstream_response_body_seen: bool,
    pub needs_completion_signal: bool,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub struct ProxySession {
    pub id: uuid::Uuid,
}

impl ProxyContext {
    pub fn new(config: ::std::sync::Arc<crate::config::Config>) -> Self {
        Self {
            config,
            modules_metrics: Vec::new(),
            valid_request_information: None,
            observed_request: None,
            routed_request: None,
            req_start: ::std::time::Instant::now(),
            req_id: uuid::Uuid::new_v4(),
            proxy_decision: None,
            parsed_cookie: None,
            needs_session_cookie: false,
            http_request: None,
            downstream_ws_kind: DownstreamWebsocket::None,
            session_id_bytes: [0u8; 16],
            buffered_request_body: None,
            fallback_error_page_body: None,
            upstream_response_body_seen: false,
            needs_completion_signal: false,
        }
    }
}

impl Drop for ProxyContext {
    fn drop(&mut self) {
        if self.needs_completion_signal {
            crate::metrics::runtime_signals::RUNTIME_SIGNALS.request_finished();
            self.needs_completion_signal = false;
        }
    }
}

#[derive(Debug, Clone)]
/// Fully validated request information after routing has matched a destination.
///
/// Unlike `PartialRequestInformation`, this struct includes the resolved routing
/// destination (`req_match`), the session identifier, and a shared config Arc.
pub struct ValidRequestInformation {
    pub scheme: http::uri::Scheme,
    pub host: smol_str::SmolStr,
    pub path: ksbh_types::KsbhStr,
    pub method: http::Method,
    pub client_information: PartialClientInformation,
    pub config: ::std::sync::Arc<crate::config::Config>,
    pub req_match: crate::routing::RequestMatch,
    pub session_id: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct PartialRequestInformation {
    pub http_request_info: ksbh_types::prelude::HttpRequest,
    pub client_information: PartialClientInformation,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Request-scoped client identity captured before routing.
pub struct ClientInformation {
    pub ip: ::std::net::IpAddr,
    pub header_hash: [u8; 32],
    pub reputation_key: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct ObservedRequest {
    pub req_id: uuid::Uuid,
    pub started_at: ::std::time::Instant,
    pub client: ClientInformation,
    pub session_id: uuid::Uuid,
    pub http_request: ksbh_types::requests::http_request::HttpRequest,
    pub is_websocket_handshake: bool,
}

#[derive(Debug, Clone)]
pub struct RoutedRequest {
    pub route_match: crate::routing::RequestMatch,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PartialClientInformation {
    pub ip: ::std::net::IpAddr,
    pub user_agent: Option<ksbh_types::KsbhStr>,
}

impl ValidRequestInformation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scheme: http::uri::Scheme,
        host: smol_str::SmolStr,
        path: ksbh_types::KsbhStr,
        method: http::Method,
        client_information: PartialClientInformation,
        config: ::std::sync::Arc<crate::config::Config>,
        req_match: crate::routing::RequestMatch,
        session_id: uuid::Uuid,
    ) -> Self {
        Self {
            scheme,
            host,
            path,
            method,
            client_information,
            config,
            req_match,
            session_id,
        }
    }
}

impl PartialClientInformation {
    pub fn new_from_session(
        session: &dyn ksbh_types::prelude::ProxyProviderSession,
        config: &crate::Config,
    ) -> Option<Self> {
        let trust_forwarded_headers = config.trusts_forwarded_headers_from(session.client_addr());
        Some(Self {
            ip: crate::utils::get_client_ip_from_session(session, trust_forwarded_headers)?,
            user_agent: match session
                .header_map()
                .get(http::header::USER_AGENT)
                .map(|ua| ua.to_str().ok())
            {
                Some(ua) => ua.map(ksbh_types::KsbhStr::new),
                None => None,
            },
        })
    }
}

impl ClientInformation {
    pub fn stable_filtered_header_hash(headers: &http::HeaderMap) -> [u8; 32] {
        let mut normalized_headers = headers
            .iter()
            .filter_map(|(name, value)| {
                let name = name.as_str().to_ascii_lowercase();

                if Self::is_excluded_identity_header(&name) {
                    return None;
                }

                Some((name, value.as_bytes().to_vec()))
            })
            .collect::<Vec<_>>();

        normalized_headers.sort_unstable_by(|(name_a, value_a), (name_b, value_b)| {
            name_a
                .cmp(name_b)
                .then_with(|| value_a.as_slice().cmp(value_b.as_slice()))
        });

        let mut hasher = Blake3Hasher::new();
        hasher.update(b"ksbh:headers");

        for (name, value) in normalized_headers {
            hasher.update(name.as_bytes());
            hasher.update(&[0]);
            hasher.update(&value);
            hasher.update(&[0]);
        }

        *hasher.finalize().as_bytes()
    }

    fn is_excluded_identity_header(name: &str) -> bool {
        matches!(
            name,
            "authorization"
                | "proxy-authorization"
                | "cookie"
                | "set-cookie"
                | "host"
                | "connection"
                | "upgrade"
                | "keep-alive"
                | "transfer-encoding"
                | "te"
                | "trailer"
                | "content-length"
                | "content-type"
                | "content-encoding"
                | "content-language"
                | "content-disposition"
                | "content-location"
                | "content-range"
                | "accept"
                | "accept-charset"
                | "accept-encoding"
                | "accept-language"
                | "accept-datetime"
                | "cache-control"
                | "pragma"
                | "prefer"
                | "range"
                | "if-match"
                | "if-none-match"
                | "if-modified-since"
                | "if-unmodified-since"
                | "if-range"
                | "via"
                | "forwarded"
                | "forward"
                | "x-forwarded-for"
                | "x-forwarded-host"
                | "x-forwarded-proto"
                | "x-forwarded-port"
                | "x-forwarded-server"
                | "x-forwarded-prefix"
                | "x-forwarded"
                | "x-real-ip"
                | "x-cluster-client-ip"
                | "x-original-forwarded-for"
                | "x-forwarded-client-cert"
                | "cf-connecting-ip"
                | "true-client-ip"
                | "proxy-connection"
                | "x-forwarded-scheme"
                | "x-http-method-override"
                | "x-method-override"
                | "x-redirect-by"
        )
    }

    fn reputation_key_from_parts(ip: ::std::net::IpAddr, header_hash: [u8; 32]) -> [u8; 32] {
        let mut hasher = Blake3Hasher::new();
        hasher.update(b"ksbh:reputation:identity");

        match ip {
            ::std::net::IpAddr::V4(ipv4) => {
                hasher.update(&ipv4.octets());
            }
            ::std::net::IpAddr::V6(ipv6) => {
                hasher.update(&ipv6.octets());
            }
        }

        hasher.update(&header_hash);
        *hasher.finalize().as_bytes()
    }

    fn from_parts(ip: ::std::net::IpAddr, headers: &http::HeaderMap) -> Self {
        let header_hash = Self::stable_filtered_header_hash(headers);
        let reputation_key = Self::reputation_key_from_parts(ip, header_hash);

        Self {
            ip,
            header_hash,
            reputation_key,
        }
    }

    pub fn new_from_session(
        session: &dyn ksbh_types::prelude::ProxyProviderSession,
        config: &crate::Config,
    ) -> Option<Self> {
        let trust_forwarded_headers = config.trusts_forwarded_headers_from(session.client_addr());
        let ip = crate::utils::get_client_ip_from_session(session, trust_forwarded_headers)?;
        Some(Self::from_parts(ip, session.header_map()))
    }
}

impl ::std::borrow::Borrow<::std::net::IpAddr> for ClientInformation {
    fn borrow(&self) -> &::std::net::IpAddr {
        &self.ip
    }
}

impl ::std::fmt::Display for ClientInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ip)
    }
}

impl ::std::borrow::Borrow<::std::net::IpAddr> for PartialClientInformation {
    fn borrow(&self) -> &::std::net::IpAddr {
        &self.ip
    }
}

impl ::std::fmt::Display for PartialClientInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ip)?;

        if let Some(ua) = &self.user_agent {
            write!(f, " - {}", ua)?;
        }

        Ok(())
    }
}

impl From<ClientInformation> for PartialClientInformation {
    fn from(value: ClientInformation) -> Self {
        Self {
            ip: value.ip,
            user_agent: None,
        }
    }
}

impl redis::ToRedisArgs for PartialClientInformation {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        if let Ok(encoded) = rmp_serde::to_vec(self) {
            out.write_arg(&encoded);
        }
    }
}

impl redis::ToSingleRedisArg for PartialClientInformation {}

#[cfg(test)]
mod tests {
    use super::ClientInformation;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn stable_filtered_header_hash_is_deterministic_and_order_independent() {
        let mut headers_a = http::HeaderMap::new();
        headers_a.insert("x-stable", http::HeaderValue::from_static("alpha"));
        headers_a.insert("x-another", http::HeaderValue::from_static("beta"));
        headers_a.insert(
            http::header::COOKIE,
            http::HeaderValue::from_static("session=a"),
        );
        headers_a.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("secret-a"),
        );

        let mut headers_b = http::HeaderMap::new();
        headers_b.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("secret-b"),
        );
        headers_b.insert("x-another", http::HeaderValue::from_static("beta"));
        headers_b.insert(
            http::header::COOKIE,
            http::HeaderValue::from_static("session=b"),
        );
        headers_b.insert("x-stable", http::HeaderValue::from_static("alpha"));

        assert_eq!(
            ClientInformation::stable_filtered_header_hash(&headers_a),
            ClientInformation::stable_filtered_header_hash(&headers_b)
        );
    }

    #[test]
    fn stable_filtered_header_hash_ignores_excluded_headers() {
        let mut headers_a = http::HeaderMap::new();
        headers_a.insert("x-stable", http::HeaderValue::from_static("alpha"));
        headers_a.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("one"),
        );

        let mut headers_b = http::HeaderMap::new();
        headers_b.insert("x-stable", http::HeaderValue::from_static("alpha"));
        headers_b.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("two"),
        );

        assert_eq!(
            ClientInformation::stable_filtered_header_hash(&headers_a),
            ClientInformation::stable_filtered_header_hash(&headers_b)
        );
    }

    #[test]
    fn stable_filtered_header_hash_changes_when_included_header_changes() {
        let mut headers_a = http::HeaderMap::new();
        headers_a.insert("x-stable", http::HeaderValue::from_static("alpha"));

        let mut headers_b = http::HeaderMap::new();
        headers_b.insert("x-stable", http::HeaderValue::from_static("bravo"));

        assert_ne!(
            ClientInformation::stable_filtered_header_hash(&headers_a),
            ClientInformation::stable_filtered_header_hash(&headers_b)
        );
    }

    #[test]
    fn reputation_key_changes_when_ip_changes_with_same_headers() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-stable", http::HeaderValue::from_static("alpha"));
        headers.insert("x-noisy", http::HeaderValue::from_static("ignored"));

        let client_a =
            ClientInformation::from_parts(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)), &headers);
        let client_b =
            ClientInformation::from_parts(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 11)), &headers);

        assert_eq!(client_a.header_hash, client_b.header_hash);
        assert_ne!(client_a.reputation_key, client_b.reputation_key);
    }
}
