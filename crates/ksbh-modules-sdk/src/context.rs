/// Module request context passed to the module's `process` function.
///
/// Contains all data and handles the module needs to process a request:
/// - Request data (headers, body, path, method)
/// - Session storage for persisting data across requests
/// - Metrics handle for score tracking
/// - Logger for emitting log messages
pub struct RequestContext<'a> {
    /// Module configuration key-value pairs from the module definition.
    pub config: ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr>,
    /// HTTP request headers.
    pub headers: http::HeaderMap,
    /// Parsed request information (URI, host, method, path, query params).
    pub request: RequestInfo,
    /// Request body bytes.
    pub body: &'a [u8],
    /// Handle for reading/writing session data.
    pub session: super::session::SessionHandle,
    /// Logger for emitting messages to the host's logging infrastructure.
    pub logger: super::logger::Logger,
    /// Module name as registered in the configuration.
    pub mod_name: smol_str::SmolStr,
    /// Key for metrics score tracking (typically `session_id`).
    pub metrics_key: &'a [u8],
    /// Original cookie header from the request.
    pub cookie_header: smol_str::SmolStr,
    /// Handle for reporting metrics to the host.
    pub metrics: super::metrics::MetricsHandle,
    /// Internal path prefix for module-specific endpoints.
    pub internal_path: &'a str,
}

impl<'a> RequestContext<'a> {
    pub fn session_id(&self) -> [u8; 16] {
        self.session.session_id()
    }
}

/// Parsed HTTP request information.
///
/// Provides convenient access to request components extracted from the URI.
pub struct RequestInfo {
    /// Full request URI (e.g., `https://example.com/path?query=value`).
    pub uri: smol_str::SmolStr,
    /// Host header value (e.g., `example.com`).
    pub host: smol_str::SmolStr,
    /// HTTP method (GET, POST, etc.).
    pub method: smol_str::SmolStr,
    /// Request path (e.g., `/api/users`).
    pub path: smol_str::SmolStr,
    /// Query string parameters parsed into a map.
    pub query_params: ::std::collections::HashMap<smol_str::SmolStr, smol_str::SmolStr>,
    /// Request scheme (http or https).
    pub scheme: smol_str::SmolStr,
    /// Request port number.
    pub port: u16,
    /// Whether this request is a websocket handshake.
    pub is_websocket_handshake: bool,
}
