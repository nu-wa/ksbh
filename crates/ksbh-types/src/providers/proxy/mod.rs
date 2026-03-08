#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyProviderError {
    InternalErrorDetailled(String),
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
                Self::InternalErrorDetailled(details) => details.as_str(),
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

    async fn early_request_filter(
        &self,
        session: &mut dyn ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> ProxyProviderResult;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_provider_error_internal_error_detailed() {
        let error = ProxyProviderError::InternalErrorDetailled("Something went wrong".to_string());
        assert_eq!(
            format!("{}", error),
            "ProxyProviderError: Something went wrong"
        );
    }

    #[test]
    fn test_proxy_provider_error_internal_error() {
        let error = ProxyProviderError::InternalError;
        assert_eq!(format!("{}", error), "ProxyProviderError: InternalError");
    }

    #[test]
    fn test_proxy_provider_error_parsing_error() {
        let error = ProxyProviderError::ParsingError("Invalid URI".to_string());
        assert_eq!(format!("{}", error), "ProxyProviderError: Invalid URI");
    }

    #[test]
    fn test_proxy_provider_error_route_not_found() {
        let error = ProxyProviderError::RouteNotFound;
        assert_eq!(format!("{}", error), "ProxyProviderError: RouteNotFound");
    }

    #[test]
    fn test_proxy_provider_error_debug() {
        let error = ProxyProviderError::InternalErrorDetailled("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("InternalErrorDetailled"));
    }

    #[test]
    fn test_proxy_provider_error_parsing() {
        let error = ProxyProviderError::ParsingError("test error".to_string());
        match error {
            ProxyProviderError::ParsingError(_) => (),
            _ => panic!("Expected ParsingError"),
        }
    }

    #[test]
    fn test_proxy_decision_module_replied() {
        let decision = ProxyDecision::ModuleReplied;
        assert_eq!(format!("{}", decision), "ModuleReplied");
    }

    #[test]
    fn test_proxy_decision_continue_processing() {
        let decision = ProxyDecision::ContinueProcessing;
        assert_eq!(format!("{}", decision), "ContinueProcessing");
    }

    #[test]
    fn test_proxy_decision_stop_processing() {
        let decision = ProxyDecision::StopProcessing(
            http::StatusCode::UNAUTHORIZED,
            bytes::Bytes::from("Unauthorized"),
        );
        assert_eq!(format!("{}", decision), "StopProcessing");
    }

    #[test]
    fn test_proxy_decision_clone() {
        let decision1 = ProxyDecision::ModuleReplied;
        let decision2 = decision1.clone();
        assert_eq!(decision1, decision2);
    }

    #[test]
    fn test_proxy_decision_equality() {
        let decision1 = ProxyDecision::ContinueProcessing;
        let decision2 = ProxyDecision::ContinueProcessing;
        let decision3 = ProxyDecision::ModuleReplied;
        assert_eq!(decision1, decision2);
        assert_ne!(decision1, decision3);
    }

    #[test]
    fn test_proxy_decision_debug() {
        let decision = ProxyDecision::ContinueProcessing;
        let debug_str = format!("{:?}", decision);
        assert!(debug_str.contains("ContinueProcessing"));
    }

    #[test]
    fn test_upstream_peer_new() {
        let peer = UpstreamPeer {
            address: "127.0.0.1:8080".to_string(),
        };
        assert_eq!(peer.address, "127.0.0.1:8080");
    }

    #[test]
    fn test_upstream_peer_debug() {
        let peer = UpstreamPeer {
            address: "localhost:3000".to_string(),
        };
        let debug_str = format!("{:?}", peer);
        assert!(debug_str.contains("localhost:3000"));
    }

    #[test]
    fn test_proxy_provider_result_ok() {
        let result: ProxyProviderResult = Ok(ProxyDecision::ContinueProcessing);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), ProxyDecision::ContinueProcessing));
    }

    #[test]
    fn test_proxy_provider_result_err() {
        let result: ProxyProviderResult = Err(ProxyProviderError::RouteNotFound);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProxyProviderError::RouteNotFound
        ));
    }
}
