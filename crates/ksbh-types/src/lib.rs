pub use http;

pub mod ksbh_str;
pub mod providers;
pub mod requests;

pub use ksbh_str::KsbhStr;

pub mod prelude {
    #[cfg(feature = "test-util")]
    pub use crate::providers::proxy::test_utils::{MockBuilder, MockProxyProviderSession};
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

pub type ArcHashMap<K, V> = arc_swap::ArcSwap<::std::collections::HashMap<K, V>>;
