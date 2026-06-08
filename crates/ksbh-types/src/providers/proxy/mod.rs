pub mod peer_options;

/// Errors that can occur during proxy provider operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyProviderError {
    /// Internal error with detailed description
    InternalErrorDetailed(String),
    /// Generic internal error
    InternalError,
    /// Parsing failed with details
    ParsingError(String),
    /// No matching route found
    RouteNotFound,
}

impl ::std::error::Error for ProxyProviderError {}

impl ::std::fmt::Display for ProxyProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProxyProviderError: {}",
            match self {
                Self::InternalErrorDetailed(details) => details.as_str(),
                Self::InternalError => "InternalError",
                Self::ParsingError(details) => details.as_str(),
                Self::RouteNotFound => "RouteNotFound",
            }
        )
    }
}

impl From<http::uri::InvalidUri> for ProxyProviderError {
    fn from(value: http::uri::InvalidUri) -> Self {
        Self::ParsingError(value.to_string())
    }
}

impl From<http::header::InvalidHeaderValue> for ProxyProviderError {
    fn from(value: http::header::InvalidHeaderValue) -> Self {
        Self::ParsingError(value.to_string())
    }
}

impl From<http::header::MaxSizeReached> for ProxyProviderError {
    fn from(value: http::header::MaxSizeReached) -> Self {
        Self::ParsingError(value.to_string())
    }
}

impl From<Box<pingora::Error>> for ProxyProviderError {
    fn from(value: Box<pingora::Error>) -> Self {
        Self::ParsingError(value.to_string())
    }
}

/// Decision made by a proxy provider on how to handle a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyDecision {
    /// Module handled the request and wrote a response
    ModuleReplied,
    /// Continue normal proxy processing
    ContinueProcessing,
    /// Stop processing with the given status and body
    StopProcessing(http::StatusCode, bytes::Bytes),
}

/// Represents an upstream peer to proxy requests to.
#[derive(Debug)]
pub struct UpstreamPeer {
    /// The address of the upstream peer
    pub address: String,
    pub peer_options: Option<crate::providers::proxy::peer_options::PeerOptions>,
}

impl ::std::fmt::Display for ProxyDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::ModuleReplied => "ModuleReplied",
                Self::ContinueProcessing => "ContinueProcessing",
                Self::StopProcessing(_, _) => "StopProcessing",
            }
        )
    }
}

pub type ProxyProviderResult = Result<ProxyDecision, ProxyProviderError>;

/// Session abstraction for proxy provider operations.
#[async_trait::async_trait]
pub trait ProxyProviderSession: Send + Sync {
    fn headers(&self) -> http::request::Parts;
    fn header_map(&self) -> &http::HeaderMap;
    fn get_header(&self, header_name: http::HeaderName) -> Option<&http::header::HeaderValue>;
    fn set_request_uri(&mut self, uri: http::Uri);
    fn server_addr(&self) -> Option<::std::net::SocketAddr>;

    fn response_written(&self) -> bool;
    fn response_status(&self) -> Option<http::StatusCode>;

    fn response_sent(&self) -> bool;

    fn client_addr(&self) -> Option<::std::net::IpAddr>;

    async fn write_response(
        &mut self,
        response: http::Response<bytes::Bytes>,
    ) -> Result<(), ProxyProviderError>;

    async fn read_request_body(&mut self) -> Result<Option<bytes::Bytes>, ProxyProviderError>;
}
