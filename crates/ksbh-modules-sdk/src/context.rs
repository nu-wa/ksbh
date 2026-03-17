pub struct RequestContext<'a> {
    pub config: ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr>,
    pub headers: http::HeaderMap,
    pub request: RequestInfo,
    pub body: &'a [u8],
    pub session: super::session::SessionHandle,
    pub logger: super::logger::Logger,
    pub mod_name: smol_str::SmolStr,
    pub metrics_key: &'a [u8],
    pub cookie_header: smol_str::SmolStr,
    pub metrics: super::metrics::MetricsHandle,
    pub internal_path: &'a str,
}

impl<'a> RequestContext<'a> {
    pub fn session_id(&self) -> [u8; 16] {
        self.session.session_id()
    }
}

pub struct RequestInfo {
    pub uri: smol_str::SmolStr,
    pub host: smol_str::SmolStr,
    pub method: smol_str::SmolStr,
    pub path: smol_str::SmolStr,
    pub query_params: ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr>,
    pub scheme: smol_str::SmolStr,
    pub port: u16,
}
