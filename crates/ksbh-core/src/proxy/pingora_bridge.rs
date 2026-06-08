use crate::proxy::{DownstreamWebsocket, ProxyContext, ProxyService};
use ksbh_types::prelude::{ProxyDecision, ProxyProviderError, ProxyProviderSession};
use pingora_core::upstreams::peer::{HttpPeer, PeerOptions};
use pingora_core::{Error, ErrorSource, ErrorType, ImmutStr, Result};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{FailToProxy, ProxyHttp, Session};

pub struct PingoraSessionWrapper<'a> {
    pub session: &'a mut Session,
}

impl<'a> PingoraSessionWrapper<'a> {
    pub fn new(session: &'a mut Session) -> Self {
        Self { session }
    }
}

#[async_trait::async_trait]
impl<'a> ProxyProviderSession for PingoraSessionWrapper<'a> {
    fn headers(&self) -> http::request::Parts {
        self.session.req_header().as_owned_parts()
    }

    fn header_map(&self) -> &http::HeaderMap {
        &self.session.req_header().headers
    }

    fn get_header(&self, header: http::HeaderName) -> Option<&http::HeaderValue> {
        self.session.get_header(header)
    }

    fn client_addr(&self) -> Option<::std::net::IpAddr> {
        match self.session.client_addr() {
            None => None,
            Some(sock_addr) => sock_addr.as_inet().map(|sock_addr| sock_addr.ip()),
        }
    }

    fn response_written(&self) -> bool {
        self.session.response_written().is_some()
    }

    fn response_status(&self) -> Option<http::StatusCode> {
        self.session
            .response_written()
            .map(|response| response.status)
    }

    fn set_request_uri(&mut self, uri: http::Uri) {
        self.session.req_header_mut().set_uri(uri);
    }

    fn server_addr(&self) -> Option<::std::net::SocketAddr> {
        self.session
            .server_addr()
            .and_then(|addr| addr.as_inet().copied())
    }

    fn response_sent(&self) -> bool {
        self.session.body_bytes_sent() > 0 || self.response_written()
    }

    async fn write_response(
        &mut self,
        response: http::Response<bytes::Bytes>,
    ) -> Result<(), ProxyProviderError> {
        let headers = response.headers();
        let mut pingora_headers = ResponseHeader::build(response.status(), None)?;

        for (header_name, header_value) in headers {
            pingora_headers.insert_header(header_name, header_value)?;
        }

        self.session
            .write_response_header(Box::new(pingora_headers), false)
            .await?;
        self.session
            .write_response_body(Some(response.body().to_owned()), true)
            .await?;
        Ok(())
    }

    async fn read_request_body(
        &mut self,
    ) -> Result<Option<bytes::Bytes>, ProxyProviderError> {
        // If request has no body, `self.session.read_request_body` will timeout.
        if self.session.is_body_empty() {
            return Ok(None);
        }

        let mut body_buffer = bytes::BytesMut::new();
        loop {
            match self.session.read_request_body().await {
                Ok(Some(chunk)) => {
                    if !chunk.is_empty() {
                        body_buffer.extend_from_slice(chunk.as_ref());
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return Err(ProxyProviderError::InternalErrorDetailed(e.to_string()));
                }
            }
        }

        if body_buffer.is_empty() {
            Ok(None)
        } else {
            Ok(Some(body_buffer.freeze()))
        }
    }
}

fn to_pingora_err(e: ProxyProviderError) -> Box<Error> {
    Error::create(
        ErrorType::Custom("InternalError"),
        ErrorSource::Internal,
        Some(ImmutStr::Owned(e.to_string().into())),
        None,
    )
}

impl ProxyService {
    pub fn header_has_token(
        headers: &http::HeaderMap,
        name: impl http::header::AsHeaderName,
        token: &str,
    ) -> bool {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case(token))
            })
            .unwrap_or(false)
    }

    fn is_h1_websocket_upgrade(headers: &http::HeaderMap) -> bool {
        if !Self::header_has_token(headers, http::header::UPGRADE, "websocket") {
            return false;
        }

        Self::header_has_token(headers, http::header::CONNECTION, "upgrade")
            || headers.contains_key("Sec-WebSocket-Key")
    }

    fn classify_downstream_websocket(pingora_session: &Session) -> DownstreamWebsocket {
        let parts = pingora_session.req_header().as_ref();

        if Self::is_h1_websocket_upgrade(&parts.headers) {
            return DownstreamWebsocket::H1Upgrade;
        }

        DownstreamWebsocket::None
    }

    /// Applies a [`ProxyDecision`] to a session: short-circuits the request
    /// (writing the response when [`ProxyDecision::StopProcessing`] is returned)
    /// and stores the decision in the request context for later stages.
    ///
    /// Returns `Ok(true)` when the caller should stop further processing
    /// (either the module already replied or a stop response was written).
    async fn apply_decision(
        decision: ProxyDecision,
        session: &mut PingoraSessionWrapper<'_>,
        ctx: &mut ProxyContext,
    ) -> Result<bool> {
        match decision {
            ProxyDecision::ContinueProcessing => Ok(false),
            ProxyDecision::ModuleReplied => {
                ctx.proxy_decision = Some(ProxyDecision::ModuleReplied);
                Ok(true)
            }
            ProxyDecision::StopProcessing(status, body) => {
                ctx.proxy_decision = Some(ProxyDecision::StopProcessing(status, body.clone()));
                let response = http::Response::builder()
                    .status(status)
                    .body(body)
                    .map_err(|e| to_pingora_err(ProxyProviderError::InternalErrorDetailed(e.to_string())))?;
                ProxyProviderSession::write_response(session, response)
                    .await
                    .map_err(to_pingora_err)?;
                Ok(true)
            }
        }
    }
}

#[async_trait::async_trait]
impl ProxyHttp for ProxyService {
    type CTX = ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        self.new_context()
    }

    async fn early_request_filter(
        &self,
        pingora_session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        ctx.downstream_ws_kind = Self::classify_downstream_websocket(pingora_session);

        let mut session = PingoraSessionWrapper::new(pingora_session);
        let decision = self
            .early_request_filter(&mut session, ctx)
            .await
            .map_err(to_pingora_err)?;

        Self::apply_decision(decision, &mut session, ctx).await.map(|_| ())
    }

    async fn request_body_filter(
        &self,
        _pingora_session: &mut Session,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let should_inject_buffered_body =
            body.as_ref().map(|chunk| chunk.is_empty()).unwrap_or(true);

        if should_inject_buffered_body && let Some(buffered_body) = ctx.buffered_request_body.take()
        {
            *body = Some(buffered_body);
            return Ok(());
        }

        if end_of_stream {
            ctx.buffered_request_body = None;
        }

        Ok(())
    }

    async fn request_filter(
        &self,
        pingora_session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<bool> {
        if ctx.proxy_decision.is_some() || pingora_session.response_written().is_some() {
            return Ok(true);
        }

        // Enable retry buffering before modules read the request body so consumed
        // bytes are still available for upstream forwarding.
        pingora_session.as_mut().enable_retry_buffering();
        let mut session = PingoraSessionWrapper::new(pingora_session);
        let decision = self
            .request_filter(&mut session, ctx)
            .await
            .map_err(to_pingora_err)?;

        Self::apply_decision(decision, &mut session, ctx).await
    }

    async fn logging(
        &self,
        session: &mut Session,
        pingora_error: Option<&Error>,
        ctx: &mut Self::CTX,
    ) {
        let mut session = PingoraSessionWrapper::new(session);
        let error = pingora_error
            .map(|e| ProxyProviderError::InternalErrorDetailed(e.to_string()));

        self.logging(&mut session, error.as_ref(), ctx).await;
    }

    async fn upstream_peer(
        &self,
        pingora_session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let mut session = PingoraSessionWrapper::new(pingora_session);

        match self.upstream_peer(&mut session, ctx).await {
            Ok(upstream) => {
                tracing::debug!("got upstream: {:?}", upstream);
                let mut https = false;
                let mut sni = upstream.address.clone();
                let mut peer_options = PeerOptions::new();

                if let Some(upstream_peer_options) = &upstream.peer_options {
                    peer_options.verify_cert = upstream_peer_options.verify_cert;
                    peer_options.verify_hostname = upstream_peer_options.verify_cert;
                    peer_options.alternative_cn = upstream_peer_options
                        .altnerative_names
                        .first()
                        .cloned()
                        .map(|s| s.to_string());

                    if let Some(alternative_sni) = &upstream_peer_options.sni {
                        sni = alternative_sni.to_string();
                    }

                    https = upstream_peer_options.sni.is_some()
                        || !upstream_peer_options.altnerative_names.is_empty();
                }

                let mut http_peer = HttpPeer::new(upstream.address.as_str(), https, sni);

                http_peer.options = peer_options;

                Ok(Box::new(http_peer))
            }
            Err(e) => Err(to_pingora_err(e)),
        }
    }

    async fn upstream_request_filter(
        &self,
        pingora_session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let mut session = PingoraSessionWrapper::new(pingora_session);

        match self
            .upstream_request_filter(&mut session, upstream_request, ctx)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(to_pingora_err(e)),
        }
    }

    async fn response_filter(
        &self,
        pingora_session: &mut Session,
        pingora_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let mut session = PingoraSessionWrapper::new(pingora_session);
        let reason_phrase = pingora_response
            .get_reason_phrase()
            .map(::std::string::String::from);
        let mut response_parts = pingora_response.as_owned_parts();

        match self
            .response_filter(&mut session, &mut response_parts, ctx)
            .await
        {
            Ok(_) => {
                let mut rebuilt_response = ResponseHeader::from(response_parts);
                rebuilt_response.set_reason_phrase(reason_phrase.as_deref())?;

                *pingora_response = rebuilt_response;

                Ok(())
            }
            Err(e) => Err(to_pingora_err(e)),
        }
    }

    async fn upstream_response_filter(
        &self,
        _pingora_session: &mut Session,
        pingora_upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        let status = pingora_upstream_response.status;
        let has_explicit_empty_body = pingora_upstream_response
            .headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim() == "0")
            .unwrap_or(false);
        if (status.is_client_error() || status.is_server_error()) && has_explicit_empty_body {
            return Err(Error::create(
                ErrorType::HTTPStatus(status.as_u16()),
                ErrorSource::Upstream,
                Some(ImmutStr::Owned(
                    "upstream returned error status with explicit empty body".into(),
                )),
                None,
            ));
        }

        Ok(())
    }

    fn response_body_filter(
        &self,
        _pingora_session: &mut Session,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> Result<Option<std::time::Duration>> {
        self.response_body_filter(body, end_of_stream, ctx)
            .map_err(to_pingora_err)?;

        Ok(None)
    }

    async fn fail_to_proxy(
        &self,
        pingora_session: &mut Session,
        pingora_error: &Error,
        ctx: &mut Self::CTX,
    ) -> FailToProxy {
        let error_code = match pingora_error.etype() {
            ErrorType::HTTPStatus(code) => *code,
            _ => match pingora_error.esource() {
                ErrorSource::Upstream => http::StatusCode::BAD_GATEWAY.as_u16(),
                ErrorSource::Downstream => match pingora_error.etype() {
                    ErrorType::WriteError
                    | ErrorType::ReadError
                    | ErrorType::ConnectionClosed => 0,
                    _ => http::StatusCode::BAD_REQUEST.as_u16(),
                },
                ErrorSource::Internal | ErrorSource::Unset => {
                    http::StatusCode::INTERNAL_SERVER_ERROR.as_u16()
                }
            },
        };

        let mut session = PingoraSessionWrapper::new(pingora_session);
        let handled_by_provider = match self
            .fail_to_proxy(&mut session, error_code, ctx)
            .await
        {
            Ok(handled) => handled,
            Err(error) => {
                tracing::error!("proxy provider fail_to_proxy returned error: {}", error);
                false
            }
        };

        if !handled_by_provider
            && error_code > 0
            && let Err(write_error) = session.session.respond_error(error_code).await
        {
            tracing::error!("failed to send error response to downstream: {write_error}");
        }

        FailToProxy {
            error_code,
            can_reuse_downstream: false,
        }
    }
}
