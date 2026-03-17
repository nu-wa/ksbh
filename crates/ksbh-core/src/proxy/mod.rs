pub mod service;
pub(super) mod service_request_filter;

pub use service::ProxyService;

/// A [`ProxyConfiguration`](ProxyConfiguration) represents a configuration for a hostname.
///
/// Plugins and modules will be called in order that they're defined in the configuration file. For
/// example for an [Ingress]() with the annotation: `ksbh.app/plugins: "a, b, c"`, the order
/// will be `a->b->c->backend`.
#[derive(Debug)]
pub struct ProxyContext {
    pub config: ::std::sync::Arc<crate::config::Config>,
    pub modules_metrics: Vec<crate::metrics::module_metric::ModuleMetric>,
    pub valid_request_information: Option<ValidRequestInformation>,
    pub req_start: ::std::time::Instant,
    pub req_id: uuid::Uuid,
    pub proxy_decision: Option<ksbh_types::prelude::ProxyDecision>,
    pub parsed_cookie: Option<crate::cookies::ProxyCookie>,
    pub http_request: Option<ksbh_types::requests::http_request::HttpRequest>,
    pub session_id_bytes: [u8; 16],
    pub metrics_key: Vec<u8>,
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
            http_request: None,
            session_id_bytes: [0u8; 16],
            metrics_key: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidRequestInformation {
    pub host: smol_str::SmolStr,
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
    pub fn new(
        host: smol_str::SmolStr,
        client_information: PartialClientInformation,
        config: ::std::sync::Arc<crate::config::Config>,
        req_match: crate::routing::RequestMatch,
        session_id: uuid::Uuid,
    ) -> Self {
        Self {
            host,
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
    ) -> Option<Self> {
        Some(Self {
            ip: crate::utils::get_client_ip_from_session(session)?,
            user_agent: match session
                .headers()
                .headers
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
    ) -> Option<Self> {
        Some(Self {
            ip: crate::utils::get_client_ip_from_session(session)?,
            user_agent: smol_str::SmolStr::new(
                session
                    .headers()
                    .headers
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
