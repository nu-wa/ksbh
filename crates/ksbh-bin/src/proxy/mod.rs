pub mod proxy_service;

pub struct PingoraSessionWrapper<'a> {
    pub session: &'a mut pingora::proxy::Session,
}

pub struct PingoraWrapper<P> {
    provider: P,
}

impl<'a> PingoraSessionWrapper<'a> {
    pub fn new(session: &'a mut pingora::proxy::Session) -> Self {
        Self { session }
    }
}

#[async_trait::async_trait]
impl<'a> ksbh_types::prelude::ProxyProviderSession for PingoraSessionWrapper<'a> {
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
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        let headers = response.headers();
        let mut pingora_headers = pingora::prelude::ResponseHeader::build(response.status(), None)?;

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
    ) -> Result<Option<bytes::Bytes>, ksbh_types::prelude::ProxyProviderError> {
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
                    return Err(
                        ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                            e.to_string(),
                        ),
                    );
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

impl<P> PingoraWrapper<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    fn header_has_token(
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

    fn classify_downstream_websocket(
        pingora_session: &pingora::proxy::Session,
    ) -> ksbh_core::proxy::DownstreamWebsocketKind {
        let parts = pingora_session.req_header().as_ref();

        if Self::is_h1_websocket_upgrade(&parts.headers) {
            return ksbh_core::proxy::DownstreamWebsocketKind::H1Upgrade;
        }

        let protocol = parts
            .extensions
            .get::<h2::ext::Protocol>()
            .map(|value| value.as_str())
            .unwrap_or("");

        if parts.version == http::Version::HTTP_2
            && parts.method == http::Method::CONNECT
            && protocol.eq_ignore_ascii_case("websocket")
        {
            return ksbh_core::proxy::DownstreamWebsocketKind::H2ExtendedConnect;
        }

        ksbh_core::proxy::DownstreamWebsocketKind::None
    }

    fn parse_h1_response_status_and_headers(
        response_head: &str,
    ) -> Result<
        (
            http::StatusCode,
            Vec<(::std::string::String, ::std::string::String)>,
        ),
        ksbh_types::prelude::ProxyProviderError,
    > {
        let mut lines = response_head.lines();
        let status_line = lines.next().ok_or_else(|| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                "upstream websocket handshake missing status line".to_string(),
            )
        })?;

        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| {
                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(format!(
                    "upstream websocket handshake malformed status line: {status_line}",
                ))
            })?
            .parse::<u16>()
            .map_err(|e| {
                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(format!(
                    "upstream websocket handshake invalid status code: {e}",
                ))
            })?;
        let status = http::StatusCode::from_u16(status_code).map_err(|e| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
        })?;

        let mut headers = Vec::new();
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }

        Ok((status, headers))
    }

    fn is_hop_by_hop_header_name(name: &str) -> bool {
        name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("proxy-connection")
            || name.eq_ignore_ascii_case("keep-alive")
            || name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("te")
            || name.eq_ignore_ascii_case("trailer")
            || name.eq_ignore_ascii_case("upgrade")
    }

    async fn bridge_h2_websocket_to_h1_upstream(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        ctx: &mut ksbh_core::proxy::ProxyContext,
        plan: &ksbh_core::proxy::WebsocketTunnelPlan,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError>
    where
        P: ksbh_types::prelude::ProxyProvider<ProxyContext = ksbh_core::proxy::ProxyContext>,
    {
        let connect_timeout = tokio::time::Duration::from_secs(5);
        let mut upstream = tokio::time::timeout(
            connect_timeout,
            tokio::net::TcpStream::connect(plan.upstream_addr.as_str()),
        )
        .await
        .map_err(|_| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                "timeout while connecting websocket upstream".to_string(),
            )
        })?
        .map_err(|e| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
        })?;

        let downstream_headers = &pingora_session.req_header().as_ref().headers;
        let mut upstream_request = pingora::http::RequestHeader::build_no_case(
            "GET",
            plan.path_and_query.as_bytes(),
            Some(downstream_headers.len() + 8),
        )
        .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        upstream_request
            .insert_header(http::header::HOST, plan.host.as_str())
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        upstream_request
            .insert_header(http::header::UPGRADE, "websocket")
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        upstream_request
            .insert_header(http::header::CONNECTION, "Upgrade")
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        for (name, value) in downstream_headers {
            let name_str = name.as_str();
            if name == http::header::HOST
                || Self::is_hop_by_hop_header_name(name_str)
                || name_str.eq_ignore_ascii_case(ksbh_core::constants::HEADER_X_FORWARDED_FOR)
                || name_str.eq_ignore_ascii_case(ksbh_core::constants::HEADER_X_FORWARDED_PROTO)
                || name_str.eq_ignore_ascii_case(ksbh_core::constants::HEADER_X_FORWARDED_SSL)
                || name_str.eq_ignore_ascii_case(ksbh_core::constants::HEADER_X_FORWARDED_HOST)
            {
                continue;
            }
            upstream_request
                .append_header(name.clone(), value.clone())
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        // RFC 6455 handshake fields are mandatory on the H1 side. Some H2 extended
        // CONNECT clients do not include them, so ensure they exist before bridging.
        if !upstream_request
            .headers
            .contains_key("Sec-WebSocket-Version")
        {
            upstream_request
                .insert_header("Sec-WebSocket-Version", "13")
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }
        if !upstream_request.headers.contains_key("Sec-WebSocket-Key") {
            let websocket_key = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                uuid::Uuid::new_v4().as_bytes(),
            );
            upstream_request
                .insert_header("Sec-WebSocket-Key", websocket_key.as_str())
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        {
            let mut session = PingoraSessionWrapper::new(pingora_session);
            self.provider
                .upstream_request_filter(&mut session, &mut upstream_request, ctx)
                .await?;
        }

        let upstream_path = upstream_request
            .uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let mut request = format!(
            "{} {} HTTP/1.1\r\n",
            upstream_request.method.as_str(),
            upstream_path
        );
        for (name, value) in &upstream_request.headers {
            if let Ok(value_str) = value.to_str() {
                request.push_str(name.as_str());
                request.push_str(": ");
                request.push_str(value_str);
                request.push_str("\r\n");
            }
        }
        request.push_str("\r\n");

        tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            tokio::io::AsyncWriteExt::write_all(&mut upstream, request.as_bytes()),
        )
        .await
        .map_err(|_| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                "timeout while writing websocket handshake to upstream".to_string(),
            )
        })?
        .map_err(|e| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
        })?;

        let mut upstream_buffer = Vec::<u8>::with_capacity(4096);
        let handshake_deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);
        let header_end = loop {
            if let Some(index) = upstream_buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
            {
                break index + 4;
            }

            let mut read_buffer = [0u8; 2048];
            let remaining = handshake_deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| {
                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                        "timeout while reading websocket handshake from upstream".to_string(),
                    )
                })?;
            let read = tokio::time::timeout(
                remaining,
                tokio::io::AsyncReadExt::read(&mut upstream, &mut read_buffer),
            )
            .await
            .map_err(|_| {
                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                    "timeout while reading websocket handshake from upstream".to_string(),
                )
            })?
            .map_err(|e| {
                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
            })?;

            if read == 0 {
                return Err(
                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                        "upstream closed websocket handshake socket".to_string(),
                    ),
                );
            }

            upstream_buffer.extend_from_slice(&read_buffer[..read]);
            if upstream_buffer.len() > 16384 {
                return Err(
                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                        "upstream websocket handshake headers too large".to_string(),
                    ),
                );
            }
        };

        let header_bytes = &upstream_buffer[..header_end];
        let header_string = ::std::str::from_utf8(header_bytes).map_err(|e| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
        })?;
        let (upstream_status, upstream_headers) =
            Self::parse_h1_response_status_and_headers(header_string)?;
        let downstream_status = if ctx.downstream_ws_kind
            == ksbh_core::proxy::DownstreamWebsocketKind::H2ExtendedConnect
            && upstream_status == http::StatusCode::SWITCHING_PROTOCOLS
        {
            http::StatusCode::OK
        } else {
            upstream_status
        };

        let mut upstream_response = pingora::http::ResponseHeader::build(
            downstream_status,
            Some(upstream_headers.len() + 8),
        )
        .map_err(|e| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
        })?;
        for (name, value) in &upstream_headers {
            if Self::is_hop_by_hop_header_name(name.as_str()) {
                continue;
            }
            let header_name =
                http::header::HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
                })?;
            upstream_response
                .append_header(
                    header_name,
                    http::HeaderValue::from_str(value.as_str())
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        let mut response_parts = upstream_response.as_owned_parts();
        {
            let mut session = PingoraSessionWrapper::new(pingora_session);
            self.provider
                .response_filter(&mut session, &mut response_parts, ctx)
                .await?;
        }
        let headers_count = response_parts.headers.len();
        let mut response =
            pingora::http::ResponseHeader::build(response_parts.status, Some(headers_count))
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        response.set_version(response_parts.version);
        for (header_name, header_value) in &response_parts.headers {
            response
                .append_header(header_name.clone(), header_value.clone())
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        pingora_session
            .write_response_header(Box::new(response), false)
            .await
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        if header_end < upstream_buffer.len() {
            pingora_session
                .write_response_body(
                    Some(bytes::Bytes::copy_from_slice(
                        &upstream_buffer[header_end..],
                    )),
                    upstream_status != http::StatusCode::SWITCHING_PROTOCOLS,
                )
                .await
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
            if upstream_status != http::StatusCode::SWITCHING_PROTOCOLS {
                return Ok(());
            }
        } else if upstream_status != http::StatusCode::SWITCHING_PROTOCOLS {
            pingora_session
                .write_response_body(None, true)
                .await
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
            return Ok(());
        }

        enum UpstreamEvent {
            Data(bytes::Bytes),
            Eof,
            Error(::std::string::String),
        }

        let (mut upstream_reader, mut upstream_writer) = tokio::io::split(upstream);
        let (upstream_tx, mut upstream_rx) = tokio::sync::mpsc::channel::<UpstreamEvent>(64);

        tokio::spawn(async move {
            let mut read_buffer = [0u8; 8192];
            loop {
                let read =
                    match tokio::io::AsyncReadExt::read(&mut upstream_reader, &mut read_buffer)
                        .await
                    {
                        Ok(read) => read,
                        Err(e) => {
                            let _ = upstream_tx.send(UpstreamEvent::Error(e.to_string())).await;
                            return;
                        }
                    };

                if read == 0 {
                    let _ = upstream_tx.send(UpstreamEvent::Eof).await;
                    return;
                }

                if upstream_tx
                    .send(UpstreamEvent::Data(bytes::Bytes::copy_from_slice(
                        &read_buffer[..read],
                    )))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        });

        let mut upstream_done = false;
        let mut downstream_done = false;

        loop {
            if upstream_done && downstream_done {
                break;
            }

            tokio::select! {
                downstream_read = pingora_session.read_request_body(), if !downstream_done => {
                    match downstream_read {
                        Ok(Some(data)) => {
                            tokio::io::AsyncWriteExt::write_all(&mut upstream_writer, data.as_ref())
                                .await
                                .map_err(|e| {
                                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                                        e.to_string(),
                                    )
                                })?;
                        }
                        Ok(None) => {
                            downstream_done = true;
                            tokio::io::AsyncWriteExt::shutdown(&mut upstream_writer)
                                .await
                                .map_err(|e| {
                                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                                        e.to_string(),
                                    )
                                })?;
                        }
                        Err(e) => {
                            return Err(
                                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                                    e.to_string(),
                                ),
                            );
                        }
                    }
                }
                upstream_event = upstream_rx.recv(), if !upstream_done => {
                    match upstream_event {
                        Some(UpstreamEvent::Data(chunk)) => {
                            pingora_session
                                .write_response_body(Some(chunk), false)
                                .await
                                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
                        }
                        Some(UpstreamEvent::Eof) | None => {
                            upstream_done = true;
                            pingora_session
                                .write_response_body(None, true)
                                .await
                                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
                        }
                        Some(UpstreamEvent::Error(e)) => {
                            return Err(
                                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e),
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<P> pingora::proxy::ProxyHttp for PingoraWrapper<P>
where
    P: ksbh_types::prelude::ProxyProvider<ProxyContext = ksbh_core::proxy::ProxyContext>,
{
    type CTX = ksbh_core::proxy::ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        self.provider.new_context()
    }

    async fn early_request_filter(
        &self,
        _pingora_session: &mut pingora::proxy::Session,
        _ctx: &mut P::ProxyContext,
    ) -> pingora::prelude::Result<()> {
        Ok(())
    }

    async fn request_body_filter(
        &self,
        _pingora_session: &mut pingora::proxy::Session,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<()> {
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
        pingora_session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<bool> {
        // Enable retry buffering before modules read the request body so consumed
        // bytes are still available for upstream forwarding.
        pingora_session.as_mut().enable_retry_buffering();

        ctx.downstream_transport = smol_str::SmolStr::new(
            if pingora_session.req_header().version == http::Version::HTTP_2 {
                "h2"
            } else {
                "h1"
            },
        );
        ctx.downstream_ws_kind = Self::classify_downstream_websocket(pingora_session);

        let is_non_ws_connect = pingora_session.req_header().method == http::Method::CONNECT
            && ctx.downstream_ws_kind
                != ksbh_core::proxy::DownstreamWebsocketKind::H2ExtendedConnect;
        let mut session = PingoraSessionWrapper::new(pingora_session);
        let decision = self
            .provider
            .request_filter(&mut session, ctx)
            .await
            .map_err(|e| {
                pingora::Error::create(
                    pingora::ErrorType::Custom("InternalError"),
                    pingora::ErrorSource::Internal,
                    Some(pingora::ImmutStr::Owned(e.to_string().into())),
                    None,
                )
            })?;

        match decision {
            ksbh_types::prelude::ProxyDecision::ModuleReplied => Ok(true),
            ksbh_types::prelude::ProxyDecision::ContinueProcessing => {
                if is_non_ws_connect {
                    let response = http::Response::builder()
                        .status(http::StatusCode::BAD_REQUEST)
                        .body(bytes::Bytes::from_static(
                            b"CONNECT is only supported for websocket extended CONNECT",
                        ))
                        .map_err(|e| {
                            pingora::Error::create(
                                pingora::ErrorType::Custom("InternalError"),
                                pingora::ErrorSource::Internal,
                                Some(pingora::ImmutStr::Owned(e.to_string().into())),
                                None,
                            )
                        })?;
                    ctx.proxy_decision = Some(ksbh_types::prelude::ProxyDecision::StopProcessing(
                        http::StatusCode::BAD_REQUEST,
                        bytes::Bytes::from_static(
                            b"CONNECT is only supported for websocket extended CONNECT",
                        ),
                    ));
                    ksbh_types::prelude::ProxyProviderSession::write_response(
                        &mut session,
                        response,
                    )
                    .await
                    .map_err(|e| {
                        pingora::Error::create(
                            pingora::ErrorType::Custom("InternalError"),
                            pingora::ErrorSource::Internal,
                            Some(pingora::ImmutStr::Owned(e.to_string().into())),
                            None,
                        )
                    })?;
                    return Ok(true);
                }

                Ok(false)
            }
            ksbh_types::prelude::ProxyDecision::StopProcessing(status, body) => {
                ctx.proxy_decision = Some(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    status,
                    body.clone(),
                ));
                let response = http::Response::builder()
                    .status(status)
                    .body(body)
                    .map_err(|e| {
                        pingora::Error::create(
                            pingora::ErrorType::Custom("InternalError"),
                            pingora::ErrorSource::Internal,
                            Some(pingora::ImmutStr::Owned(e.to_string().into())),
                            None,
                        )
                    })?;

                ksbh_types::prelude::ProxyProviderSession::write_response(&mut session, response)
                    .await
                    .map_err(|e| {
                        pingora::Error::create(
                            pingora::ErrorType::Custom("InternalError"),
                            pingora::ErrorSource::Internal,
                            Some(pingora::ImmutStr::Owned(e.to_string().into())),
                            None,
                        )
                    })?;
                Ok(true)
            }
        }
    }

    async fn proxy_upstream_filter(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let Some(plan) = ctx.tunnel_plan.clone() else {
            return Ok(true);
        };

        if ctx.downstream_ws_kind != ksbh_core::proxy::DownstreamWebsocketKind::H2ExtendedConnect {
            return Ok(true);
        }

        self.bridge_h2_websocket_to_h1_upstream(pingora_session, ctx, &plan)
            .await
            .map_err(|e| {
                pingora::Error::create(
                    pingora::ErrorType::Custom("InternalError"),
                    pingora::ErrorSource::Internal,
                    Some(pingora::ImmutStr::Owned(e.to_string().into())),
                    None,
                )
            })?;

        Ok(false)
    }

    async fn logging(
        &self,
        session: &mut pingora::proxy::Session,
        pingora_error: Option<&pingora::Error>,
        ctx: &mut Self::CTX,
    ) {
        let mut session = PingoraSessionWrapper::new(session);
        let error = pingora_error
            .map(|e| ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string()));

        self.provider
            .logging(&mut session, error.as_ref(), ctx)
            .await;
    }

    async fn upstream_peer(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<pingora::upstreams::peer::HttpPeer>> {
        let mut session = PingoraSessionWrapper::new(pingora_session);

        match self.provider.upstream_peer(&mut session, ctx).await {
            Ok(upstream) => {
                tracing::debug!("got upstream: {:?}", upstream);
                Ok(Box::new(pingora::upstreams::peer::HttpPeer::new(
                    upstream.address.as_str(),
                    false,
                    upstream.address.clone(),
                )))
            }
            Err(e) => Err(pingora::Error::create(
                pingora::ErrorType::Custom("InternalError"),
                pingora::ErrorSource::Internal,
                Some(pingora::ImmutStr::Owned(e.to_string().into())),
                None,
            )),
        }
    }

    async fn upstream_request_filter(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        upstream_request: &mut pingora::prelude::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        let mut session = PingoraSessionWrapper::new(pingora_session);

        match self
            .provider
            .upstream_request_filter(&mut session, upstream_request, ctx)
            .await
        {
            Ok(_) => Ok(()),

            Err(e) => Err(pingora::Error::create(
                pingora::ErrorType::Custom("InternalError"),
                pingora::ErrorSource::Internal,
                Some(pingora::ImmutStr::Owned(e.to_string().into())),
                None,
            )),
        }
    }

    async fn response_filter(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        pingora_response: &mut pingora::http::ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<()> {
        let mut session = PingoraSessionWrapper::new(pingora_session);
        let reason_phrase = pingora_response
            .get_reason_phrase()
            .map(::std::string::String::from);
        let mut response_parts = pingora_response.as_owned_parts();

        match self
            .provider
            .response_filter(&mut session, &mut response_parts, ctx)
            .await
        {
            Ok(_) => {
                let mut rebuilt_response = pingora::http::ResponseHeader::from(response_parts);
                rebuilt_response.set_reason_phrase(reason_phrase.as_deref())?;

                *pingora_response = rebuilt_response;

                Ok(())
            }

            Err(e) => Err(pingora::Error::create(
                pingora::ErrorType::Custom("InternalError"),
                pingora::ErrorSource::Internal,
                Some(pingora::ImmutStr::Owned(e.to_string().into())),
                None,
            )),
        }
    }

    async fn upstream_response_filter(
        &self,
        _pingora_session: &mut pingora::proxy::Session,
        pingora_upstream_response: &mut pingora::http::ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<()> {
        let status = pingora_upstream_response.status;
        let has_explicit_empty_body = pingora_upstream_response
            .headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim() == "0")
            .unwrap_or(false);
        if (status.is_client_error() || status.is_server_error()) && has_explicit_empty_body {
            return Err(pingora::Error::create(
                pingora::ErrorType::HTTPStatus(status.as_u16()),
                pingora::ErrorSource::Upstream,
                Some(pingora::ImmutStr::Owned(
                    "upstream returned error status with explicit empty body".into(),
                )),
                None,
            ));
        }

        Ok(())
    }

    fn response_body_filter(
        &self,
        _pingora_session: &mut pingora::proxy::Session,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<Option<std::time::Duration>> {
        self.provider
            .response_body_filter(body, end_of_stream, ctx)
            .map_err(|e| {
                pingora::Error::create(
                    pingora::ErrorType::Custom("InternalError"),
                    pingora::ErrorSource::Internal,
                    Some(pingora::ImmutStr::Owned(e.to_string().into())),
                    None,
                )
            })?;

        Ok(None)
    }

    async fn fail_to_proxy(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        pingora_error: &pingora::Error,
        ctx: &mut Self::CTX,
    ) -> pingora::proxy::FailToProxy {
        let error_code = match pingora_error.etype() {
            pingora::ErrorType::HTTPStatus(code) => *code,
            _ => match pingora_error.esource() {
                pingora::ErrorSource::Upstream => http::StatusCode::BAD_GATEWAY.as_u16(),
                pingora::ErrorSource::Downstream => match pingora_error.etype() {
                    pingora::ErrorType::WriteError
                    | pingora::ErrorType::ReadError
                    | pingora::ErrorType::ConnectionClosed => 0,
                    _ => http::StatusCode::BAD_REQUEST.as_u16(),
                },
                pingora::ErrorSource::Internal | pingora::ErrorSource::Unset => {
                    http::StatusCode::INTERNAL_SERVER_ERROR.as_u16()
                }
            },
        };

        let mut session = PingoraSessionWrapper::new(pingora_session);
        let handled_by_provider = match self
            .provider
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

        pingora::proxy::FailToProxy {
            error_code,
            can_reuse_downstream: false,
        }
    }
}
