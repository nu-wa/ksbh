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

    fn get_header(&self, header: http::HeaderName) -> Option<&http::HeaderValue> {
        self.session.get_header(header)
    }

    fn client_addr(&self) -> Option<::std::net::IpAddr> {
        match self.session.client_addr() {
            None => None,
            Some(sock_addr) => sock_addr.as_inet().map(|sock_addr| sock_addr.ip()),
        }
    }

    fn response_written(&self) -> Option<http::Response<bytes::Bytes>> {
        self.session.response_written().map(|response_written| {
            http::Response::from_parts(response_written.as_owned_parts(), bytes::Bytes::new())
        })
    }

    fn set_request_uri(&mut self, uri: http::Uri) {
        self.session.req_header_mut().set_uri(uri);
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
        match self.session.read_request_body().await {
            Ok(body) => return Ok(body),
            Err(e) => {
                return Err(
                    ksbh_types::prelude::ProxyProviderError::InternalErrorDetailled(e.to_string()),
                );
            }
        }
    }
}

impl<P> PingoraWrapper<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
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
        pingora_session: &mut pingora::proxy::Session,
        ctx: &mut P::ProxyContext,
    ) -> pingora::prelude::Result<()> {
        let mut session = PingoraSessionWrapper::new(pingora_session);

        self.provider
            .early_request_filter(&mut session, ctx)
            .await
            .map_err(|e| {
                pingora::Error::create(
                    pingora::ErrorType::Custom("InternalErrror"),
                    pingora::ErrorSource::Internal,
                    Some(pingora::ImmutStr::Owned(e.to_string().into())),
                    None,
                )
            })?;

        Ok(())
    }

    async fn request_filter(
        &self,
        pingora_session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<bool> {
        let mut session = PingoraSessionWrapper::new(pingora_session);

        if ksbh_types::prelude::ProxyDecision::ModuleReplied
            == self
                .provider
                .request_filter(&mut session, ctx)
                .await
                .map_err(|e| {
                    pingora::Error::create(
                        pingora::ErrorType::Custom("InternalErrror"),
                        pingora::ErrorSource::Internal,
                        Some(pingora::ImmutStr::Owned(e.to_string().into())),
                        None,
                    )
                })?
        {
            return Ok(true);
        }

        Ok(false)
    }

    async fn logging(
        &self,
        session: &mut pingora::proxy::Session,
        pingora_error: Option<&pingora::Error>,
        ctx: &mut Self::CTX,
    ) {
        let mut session = PingoraSessionWrapper::new(session);
        let error = pingora_error.map(|e| {
            ksbh_types::prelude::ProxyProviderError::InternalErrorDetailled(e.to_string())
        });

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
                pingora::ErrorType::Custom("InternalErrror"),
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
                pingora::ErrorType::Custom("InternalErrror"),
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

        match self
            .provider
            .response_filter(&mut session, pingora_response, ctx)
            .await
        {
            Ok(_) => Ok(()),

            Err(e) => Err(pingora::Error::create(
                pingora::ErrorType::Custom("InternalErrror"),
                pingora::ErrorSource::Internal,
                Some(pingora::ImmutStr::Owned(e.to_string().into())),
                None,
            )),
        }
    }
}
