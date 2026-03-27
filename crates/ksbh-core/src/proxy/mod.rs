pub mod service;
pub(super) mod service_request_filter;

pub use service::ProxyService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownstreamWebsocketKind {
    None,
    H1Upgrade,
    H2ExtendedConnect,
}

#[derive(Debug, Clone)]
pub struct WebsocketTunnelPlan {
    pub upstream_addr: ::std::string::String,
    pub host: smol_str::SmolStr,
    pub path_and_query: ksbh_types::KsbhStr,
}

/// A [`ProxyConfiguration`](ProxyConfiguration) represents a configuration for a hostname.
///
/// Modules execute in two phases:
/// global modules first, sorted by descending weight, then ingress modules sorted by descending
/// weight for the matched ingress.
#[derive(Debug)]
/// Context passed through the entire proxy request lifecycle.
///
/// Contains configuration, parsed request data, module metrics, session state, and
/// the accumulated metrics key used for Redis-based score tracking.
pub struct ProxyContext {
    pub config: ::std::sync::Arc<crate::config::Config>,
    pub modules_metrics: Vec<crate::metrics::module_metric::ModuleMetric>,
    pub valid_request_information: Option<ValidRequestInformation>,
    pub req_start: ::std::time::Instant,
    pub req_id: uuid::Uuid,
    pub proxy_decision: Option<ksbh_types::prelude::ProxyDecision>,
    pub parsed_cookie: Option<crate::cookies::ProxyCookie>,
    pub needs_session_cookie: bool,
    pub http_request: Option<ksbh_types::requests::http_request::HttpRequest>,
    pub downstream_ws_kind: DownstreamWebsocketKind,
    pub downstream_transport: smol_str::SmolStr,
    pub tunnel_plan: Option<WebsocketTunnelPlan>,
    pub session_id_bytes: [u8; 16],
    pub metrics_key: Vec<u8>,
    pub buffered_request_body: Option<bytes::Bytes>,
    pub fallback_error_page_body: Option<bytes::Bytes>,
    pub upstream_response_body_seen: bool,
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
            req_start: ::std::time::Instant::now(),
            req_id: uuid::Uuid::new_v4(),
            proxy_decision: None,
            parsed_cookie: None,
            needs_session_cookie: false,
            http_request: None,
            downstream_ws_kind: DownstreamWebsocketKind::None,
            downstream_transport: smol_str::SmolStr::new("h1"),
            tunnel_plan: None,
            session_id_bytes: [0u8; 16],
            metrics_key: Vec::new(),
            buffered_request_body: None,
            fallback_error_page_body: None,
            upstream_response_body_seen: false,
        }
    }
}

#[derive(Debug, Clone)]
/// Fully validated request information after routing has matched a backend.
///
/// Unlike `PartialRequestInformation`, this struct includes the resolved routing
/// destination (`req_match`), the session identifier, and a shared config Arc.
pub struct ValidRequestInformation {
    pub scheme: ksbh_types::prelude::HttpScheme,
    pub host: smol_str::SmolStr,
    pub path: ksbh_types::KsbhStr,
    pub method: ksbh_types::prelude::HttpMethod,
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

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Client identification with IP address and mandatory user-agent string.
///
/// Unlike `PartialClientInformation`, the user-agent is guaranteed to be present.
/// Implements `Hash` and `Borrow<IpAddr>` for use as a map key.
pub struct ClientInformation {
    pub ip: ::std::net::IpAddr,
    pub user_agent: smol_str::SmolStr,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PartialClientInformation {
    pub ip: ::std::net::IpAddr,
    pub user_agent: Option<ksbh_types::KsbhStr>,
}

impl ValidRequestInformation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scheme: ksbh_types::prelude::HttpScheme,
        host: smol_str::SmolStr,
        path: ksbh_types::KsbhStr,
        method: ksbh_types::prelude::HttpMethod,
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
    pub fn new_from_session(
        session: &dyn ksbh_types::prelude::ProxyProviderSession,
        config: &crate::Config,
    ) -> Option<Self> {
        let trust_forwarded_headers = config.trusts_forwarded_headers_from(session.client_addr());
        Some(Self {
            ip: crate::utils::get_client_ip_from_session(session, trust_forwarded_headers)?,
            user_agent: smol_str::SmolStr::new(
                session
                    .header_map()
                    .get(http::header::USER_AGENT)?
                    .to_str()
                    .ok()?,
            ),
        })
    }
}

impl ::std::borrow::Borrow<::std::net::IpAddr> for ClientInformation {
    fn borrow(&self) -> &::std::net::IpAddr {
        &self.ip
    }
}

impl ::std::fmt::Display for ClientInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.ip, self.user_agent)
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
            user_agent: Some(ksbh_types::KsbhStr::new(value.user_agent.as_str())),
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
