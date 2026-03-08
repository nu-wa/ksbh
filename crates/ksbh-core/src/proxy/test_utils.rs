#[cfg(feature = "test-util")]
use ::std::sync::Mutex;

use ksbh_types::prelude::{
    ProxyDecision, ProxyProvider, ProxyProviderError, ProxyProviderResult, ProxyProviderSession,
};
use ksbh_types::providers::proxy::UpstreamPeer;

pub struct MockUpstream {
    responses: ::std::sync::Arc<Mutex<Vec<MockResponse>>>,
    request_log: ::std::sync::Arc<Mutex<Vec<CapturedRequest>>>,
}

#[derive(Clone, Debug)]
pub struct MockResponse {
    pub status: u16,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: bytes::Bytes,
}

impl Default for MockResponse {
    fn default() -> Self {
        Self {
            status: 200,
            headers: vec![],
            body: bytes::Bytes::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CapturedRequest {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
}

pub struct MockUpstreamBuilder {
    responses: Vec<MockResponse>,
}

impl MockUpstreamBuilder {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
        }
    }

    pub fn queue_response(mut self, response: MockResponse) -> Self {
        self.responses.push(response);
        self
    }

    pub fn queue_responses(mut self, responses: Vec<MockResponse>) -> Self {
        self.responses.extend(responses);
        self
    }

    pub fn build(self) -> MockUpstream {
        MockUpstream {
            responses: ::std::sync::Arc::new(Mutex::new(self.responses)),
            request_log: ::std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MockUpstreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockUpstream {
    pub fn builder() -> MockUpstreamBuilder {
        MockUpstreamBuilder::new()
    }

    pub fn get_response(&self) -> Option<MockResponse> {
        let mut responses = self.responses.lock().expect("Lock poisoned");
        if responses.is_empty() {
            None
        } else {
            Some(responses.remove(0))
        }
    }

    pub fn log_request(&self, request: CapturedRequest) {
        let mut log = self.request_log.lock().expect("Lock poisoned");
        log.push(request);
    }

    pub fn get_requests(&self) -> Vec<CapturedRequest> {
        let log = self.request_log.lock().expect("Lock poisoned");
        log.clone()
    }

    pub fn clear_requests(&self) {
        let mut log = self.request_log.lock().expect("Lock poisoned");
        log.clear();
    }
}

pub struct MockProxy {
    upstream: Option<MockUpstream>,
    #[allow(dead_code)]
    module_configs: Vec<ModuleConfig>,
}

pub enum ModuleConfig {
    OIDC(OIDCConfig),
    RateLimit(RateLimitConfig),
    PoW(PoWConfig),
    HttpToHttps,
    RobotsTxt,
    Custom(Box<dyn crate::modules::Module>),
}

pub struct OIDCConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub session_ttl_seconds: Option<u64>,
    pub enable_refresh: Option<bool>,
}

pub struct RateLimitConfig {
    pub requests_per_second: u64,
    pub burst: Option<u64>,
}

pub struct PoWConfig {
    pub difficulty: u32,
    pub expiry_seconds: Option<u64>,
}

pub struct MockProxyBuilder {
    upstream: Option<MockUpstream>,
    module_configs: Vec<ModuleConfig>,
}

impl MockProxyBuilder {
    pub fn new() -> Self {
        Self {
            upstream: None,
            module_configs: Vec::new(),
        }
    }

    pub fn with_upstream(mut self, upstream: MockUpstream) -> Self {
        self.upstream = Some(upstream);
        self
    }

    pub fn with_module(mut self, config: ModuleConfig) -> Self {
        self.module_configs.push(config);
        self
    }

    pub fn with_oidc(mut self, config: OIDCConfig) -> Self {
        self.module_configs.push(ModuleConfig::OIDC(config));
        self
    }

    pub fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.module_configs.push(ModuleConfig::RateLimit(config));
        self
    }

    pub fn with_pow(mut self, config: PoWConfig) -> Self {
        self.module_configs.push(ModuleConfig::PoW(config));
        self
    }

    pub fn build(self) -> MockProxy {
        MockProxy {
            upstream: self.upstream,
            module_configs: self.module_configs,
        }
    }
}

impl Default for MockProxyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProxy {
    pub fn builder() -> MockProxyBuilder {
        MockProxyBuilder::new()
    }

    pub fn upstream(&self) -> Option<&MockUpstream> {
        self.upstream.as_ref()
    }
}

impl crate::proxy::ProxyContext {
    pub fn new_for_test() -> Self {
        Self::new(crate::config::Config::new_for_test())
    }
}

impl crate::config::Config {
    pub fn new_for_test() -> ::std::sync::Arc<Self> {
        ::std::sync::Arc::new(Self {
            http_port: 8080,
            https_port: 8443,
            ext_http_port: 80,
            ext_https_port: 443,
            listen_address: "0.0.0.0:8080".parse().unwrap(),
            listen_address_tls: "0.0.0.0:8443".parse().unwrap(),
            listen_address_api: "0.0.0.0:8081".parse().unwrap(),
            listen_address_prom: "0.0.0.0:9090".parse().unwrap(),
            listen_address_internal: "0.0.0.0:8082".parse().unwrap(),
            listen_address_profiling: "0.0.0.0:6060".parse().unwrap(),
            plugins_directory: ::std::path::PathBuf::from("/tmp/plugins"),
            database_url: String::new(),
            redis_url: String::from("redis://localhost:6379"),
            threads: 1,
            config_directory: ::std::path::PathBuf::from("/tmp/config"),
            serve_directory: ::std::path::PathBuf::from("/tmp/serve"),
            web_app_mode: crate::config::ConfigWebAppMode::SubPath(smol_str::SmolStr::new("/ksbh")),
            modules_internal_path: smol_str::SmolStr::new("/_ksbh_internal"),
            modules_directory: ::std::path::PathBuf::from("/tmp/modules"),
            pyroscope_url: None,
        })
    }
}

#[async_trait::async_trait]
impl ProxyProvider for MockProxy {
    type ProxyContext = crate::proxy::ProxyContext;

    fn new_context(&self) -> Self::ProxyContext {
        crate::proxy::ProxyContext::new(crate::config::Config::new_for_test())
    }

    async fn early_request_filter(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _ctx: &mut Self::ProxyContext,
    ) -> ProxyProviderResult {
        Ok(ProxyDecision::ContinueProcessing)
    }

    async fn request_filter(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _ctx: &mut Self::ProxyContext,
    ) -> ProxyProviderResult {
        Ok(ProxyDecision::ContinueProcessing)
    }

    async fn upstream_peer(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _ctx: &mut Self::ProxyContext,
    ) -> Result<UpstreamPeer, ProxyProviderError> {
        if let Some(ref upstream) = self.upstream
            && upstream.get_response().is_some()
        {
            return Ok(UpstreamPeer {
                address: "127.0.0.1:8080".to_string(),
            });
        }
        Ok(UpstreamPeer {
            address: "127.0.0.1:8080".to_string(),
        })
    }

    async fn response_filter(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _response: &mut http::response::Parts,
        _ctx: &mut Self::ProxyContext,
    ) -> Result<(), ProxyProviderError> {
        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _upstream_request: &mut pingora::prelude::RequestHeader,
        _ctx: &mut Self::ProxyContext,
    ) -> Result<(), ProxyProviderError> {
        Ok(())
    }

    async fn logging(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _error: Option<&ProxyProviderError>,
        _ctx: &mut Self::ProxyContext,
    ) {
    }
}
