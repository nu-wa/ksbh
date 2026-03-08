pub struct RequestContext<'a> {
    pub config: &'a ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr>,
    pub headers: &'a http::HeaderMap,
    pub request: RequestInfo,
    pub body: &'a [u8],
    pub session: super::session::SessionHandle,
    pub logger: super::logger::Logger,
    pub mod_name: smol_str::SmolStr,
    pub client_ip: smol_str::SmolStr,
    pub user_agent: Option<smol_str::SmolStr>,
    pub cookie_header: smol_str::SmolStr,
    pub metrics: super::metrics::MetricsHandle,
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
