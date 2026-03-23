//! This crate holds common types used in the workspace.
pub use http;

pub mod ksbh_str;
pub mod providers;
pub mod requests;

pub use ksbh_str::KsbhStr;

pub mod prelude {
    pub use crate::providers::proxy::{
        ProxyDecision, ProxyProvider, ProxyProviderError, ProxyProviderResult, ProxyProviderSession,
    };
    pub use crate::requests::{
        HttpContext, HttpMethod, HttpQuery, HttpRequest, HttpRequestError, HttpResponse, HttpScheme,
    };
}

/// Public-facing configuration for external clients.
pub struct PublicConfig {
    pub https_port: u16,
    pub http_port: u16,
}

/// Port configuration for HTTP and HTTPS listeners.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Ports {
    pub http: u16,
    pub https: u16,
}

/// Thread-safe, lock-free HashMap backed by ArcSwap.
pub type ArcHashMap<K, V> = arc_swap::ArcSwap<::std::collections::HashMap<K, V>>;
