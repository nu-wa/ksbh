/// A [`ProxyConfiguration`](ProxyConfiguration) represents a configuration for a hostname.
///
/// Plugins and modules will be called in order that they're defined in the configuration file. For
/// example for an [Ingress]() with the annotation: `ksbh.app/plugins: "a, b, c"`, the order
/// will be `a->b->c->backend`.
#[derive(Debug)]
pub struct ProxyContext {
    pub config: ::std::sync::Arc<crate::config::Config>,
    pub backend: crate::routing::ServiceBackendType,
    pub had_cookie: bool,
    pub modules_metrics: Vec<crate::metrics::module_metric::ModuleMetric>,
    pub early_request_information: Option<EarlyRequestInformation>,
    pub valid_request_information: Option<ValidRequestInformation>,
    pub partial_request_information: Option<PartialRequestInformation>,
    pub req_start: ::std::time::Instant,
    pub req_id: uuid::Uuid,
    pub proxy_decision: Option<ksbh_types::prelude::ProxyDecision>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub struct ProxySession {
    pub id: uuid::Uuid,
}

impl ProxyContext {
    pub fn new(config: ::std::sync::Arc<crate::config::Config>) -> Self {
        Self {
            config,
            backend: crate::routing::ServiceBackendType::None,
            had_cookie: false,
            modules_metrics: Vec::new(),
            early_request_information: None,
            valid_request_information: None,
            partial_request_information: None,
            req_start: ::std::time::Instant::now(),
            req_id: uuid::Uuid::new_v4(),
            proxy_decision: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EarlyRequestInformation {
    pub session: ProxySession,
    pub cookie: crate::cookies::ProxyCookie,
    pub http_request_info: ksbh_types::prelude::HttpRequest,
    pub client_information: PartialClientInformation,
    pub config: ::std::sync::Arc<crate::config::Config>,
}

#[derive(Debug, Clone)]
pub struct ValidRequestInformation {
    pub session: ProxySession,
    pub cookie: crate::cookies::ProxyCookie,
    pub http_request_info: ksbh_types::prelude::HttpRequest,
    pub client_information: PartialClientInformation,
    pub config: ::std::sync::Arc<crate::config::Config>,
    pub req_match: crate::routing::RequestMatch,
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
    pub fn new_from_early(
        early: EarlyRequestInformation,
        req_match: crate::routing::RequestMatch,
    ) -> Self {
        Self {
            req_match,
            cookie: early.cookie,
            http_request_info: early.http_request_info,
            config: early.config,
            client_information: early.client_information,
            session: early.session,
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

pub mod service;
pub mod service_early_request_filter;
pub mod service_request_filter;

pub use service::ProxyService;

impl redis::ToSingleRedisArg for PartialClientInformation {}

#[cfg(feature = "test-util")]
pub mod test_utils;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_context_new() {
        let config = crate::config::Config::new_for_test();
        let ctx = ProxyContext::new(config);
        assert_eq!(ctx.backend, crate::routing::ServiceBackendType::None);
        assert!(!ctx.had_cookie);
    }

    #[test]
    fn test_proxy_session_new() {
        let session = ProxySession {
            id: uuid::Uuid::new_v4(),
        };
        assert!(session.id != uuid::Uuid::nil());
    }

    #[test]
    fn test_client_information_new() {
        let info = ClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: smol_str::SmolStr::new("test-agent"),
        };
        assert_eq!(info.ip.to_string(), "127.0.0.1");
    }

    #[test]
    fn test_client_information_display() {
        let info = ClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: smol_str::SmolStr::new("test-agent"),
        };
        assert_eq!(format!("{}", info), "127.0.0.1 - test-agent");
    }

    #[test]
    fn test_client_information_debug() {
        let info = ClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: smol_str::SmolStr::new("test-agent"),
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("127.0.0.1"));
    }

    #[test]
    fn test_partial_client_information_new() {
        let info = PartialClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: Some(ksbh_types::KsbhStr::new("test-agent")),
        };
        assert_eq!(info.ip.to_string(), "127.0.0.1");
    }

    #[test]
    fn test_partial_client_information_display_with_ua() {
        let info = PartialClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: Some(ksbh_types::KsbhStr::new("test-agent")),
        };
        assert_eq!(format!("{}", info), "127.0.0.1 - test-agent");
    }

    #[test]
    fn test_partial_client_information_display_without_ua() {
        let info = PartialClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: None,
        };
        assert_eq!(format!("{}", info), "127.0.0.1");
    }

    #[test]
    fn test_partial_client_information_from_client() {
        let client = ClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: smol_str::SmolStr::new("test-agent"),
        };
        let partial: PartialClientInformation = client.into();
        assert_eq!(partial.ip.to_string(), "127.0.0.1");
        assert!(partial.user_agent.is_some());
    }
}
