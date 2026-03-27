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

/// Trait for implementing a proxy provider that can filter requests and responses.
#[async_trait::async_trait]
pub trait ProxyProvider: Send + Sync {
    /// Context created per-request for tracking state
    type ProxyContext: Send + ::std::fmt::Debug;

    fn new_context(&self) -> Self::ProxyContext;

    async fn request_filter(
        &self,
        session: &mut dyn ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> ProxyProviderResult;

    async fn upstream_peer(
        &self,
        session: &mut dyn ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> Result<UpstreamPeer, ProxyProviderError>;

    async fn response_filter(
        &self,
        session: &mut dyn ProxyProviderSession,
        response: &mut http::response::Parts,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ProxyProviderError>;

    fn response_body_filter(
        &self,
        _body: &mut Option<bytes::Bytes>,
        _end_of_stream: bool,
        _ctx: &mut Self::ProxyContext,
    ) -> Result<(), ProxyProviderError> {
        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        session: &mut dyn ProxyProviderSession,
        response: &mut pingora::prelude::RequestHeader,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ProxyProviderError>;

    async fn fail_to_proxy(
        &self,
        _session: &mut dyn ProxyProviderSession,
        _error_code: u16,
        _ctx: &mut Self::ProxyContext,
    ) -> Result<bool, ProxyProviderError> {
        Ok(false)
    }

    async fn logging(
        &self,
        session: &mut dyn ProxyProviderSession,
        error: Option<&ProxyProviderError>,
        ctx: &mut Self::ProxyContext,
    );
}
