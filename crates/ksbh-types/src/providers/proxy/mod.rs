#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyProviderError {
    InternalErrorDetailed(String),
    InternalError,
    ParsingError(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyDecision {
    ModuleReplied,
    ContinueProcessing,
    StopProcessing(http::StatusCode, bytes::Bytes),
}

#[derive(Debug)]
pub struct UpstreamPeer {
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

#[async_trait::async_trait]
pub trait ProxyProviderSession: Send + Sync {
    fn headers(&self) -> http::request::Parts;
    fn get_header(&self, header_name: http::HeaderName) -> Option<&http::header::HeaderValue>;
    fn set_request_uri(&mut self, uri: http::Uri);

    fn response_written(&self) -> Option<http::Response<bytes::Bytes>>;

    fn response_sent(&self) -> bool;

    fn client_addr(&self) -> Option<::std::net::IpAddr>;

    async fn write_response(
        &mut self,
        response: http::Response<bytes::Bytes>,
    ) -> Result<(), ProxyProviderError>;

    async fn read_request_body(&mut self) -> Result<Option<bytes::Bytes>, ProxyProviderError>;
}

#[async_trait::async_trait]
pub trait ProxyProvider: Send + Sync {
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

    async fn upstream_request_filter(
        &self,
        session: &mut dyn ProxyProviderSession,
        response: &mut pingora::prelude::RequestHeader,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ProxyProviderError>;

    async fn logging(
        &self,
        session: &mut dyn ProxyProviderSession,
        error: Option<&ProxyProviderError>,
        ctx: &mut Self::ProxyContext,
    );
}

#[cfg(feature = "test-util")]
pub mod test_utils;
