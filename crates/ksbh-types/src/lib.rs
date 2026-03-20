///! This crate holds common types used in the workspace.
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

pub struct PublicConfig {
    pub https_port: u16,
    pub http_port: u16,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Ports {
    pub http: u16,
    pub https: u16,
}

pub type ArcHashMap<K, V> = arc_swap::ArcSwap<::std::collections::HashMap<K, V>>;
