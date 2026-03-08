impl super::ProxyService {
    pub(super) async fn _early_request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        let client_information: crate::proxy::PartialClientInformation =
            match crate::proxy::ClientInformation::new_from_session(session) {
                Some(cli_info) => cli_info.into(),
                None => match crate::proxy::PartialClientInformation::new_from_session(session) {
                    Some(partial_cli_info) => partial_cli_info,
                    None => {
                        tracing::error!("Client has no information (user agent or ip ?)");
                        return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                            http::StatusCode::BAD_REQUEST,
                            bytes::Bytes::from_static(b"Bad Request"),
                        ));
                    }
                },
            };

        let req_id = uuid::Uuid::new_v4();

        let http_request_info = match ksbh_types::prelude::HttpRequest::new(
            &session.headers(),
            req_id,
            &self.public_config,
        ) {
            Ok(h) => {
                ctx.partial_request_information = Some(crate::proxy::PartialRequestInformation {
                    http_request_info: h.clone(),
                    client_information: client_information.clone(),
                });
                h
            }
            Err(e) => {
                tracing::error!("Could not create http_request_info {e}");
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::BAD_REQUEST,
                    bytes::Bytes::from_static(b"Bad Request"),
                ));
            }
        };

        let (proxy_cookie, had_cookie) =
            match crate::cookies::ProxyCookie::from_session(session).await {
                Ok(cookie) => (cookie, true),
                Err(e) => match e {
                    crate::cookies::ProxyCookieError::NoCookie => (
                        crate::cookies::ProxyCookie::new(
                            http_request_info.host.as_str(),
                            None,
                            uuid::Uuid::new_v4(),
                        ),
                        false,
                    ),

                    _ => {
                        tracing::error!("Could not retrieve cookie from session {e}");
                        return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                            http::StatusCode::BAD_REQUEST,
                            bytes::Bytes::from_static(b"Bad Request"),
                        ));
                    }
                },
            };

        ctx.had_cookie = had_cookie;

        let req_match = self.hosts.find_route(&http_request_info);

        let early_request_info = crate::proxy::EarlyRequestInformation {
            session: crate::proxy::ProxySession {
                id: proxy_cookie.session_id,
            },
            cookie: proxy_cookie,
            http_request_info,
            config: ctx.config.clone(),
            client_information,
        };

        let req_match = match req_match {
            Some(rm) => rm,
            None => {
                ctx.early_request_information = Some(early_request_info);
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::NOT_FOUND,
                    bytes::Bytes::from_static(b"No request req_match"),
                ));
            }
        };

        ctx.backend = req_match.backend.clone();
        ctx.valid_request_information = Some(
            crate::proxy::ValidRequestInformation::new_from_early(early_request_info, req_match),
        );

        Ok(ksbh_types::prelude::ProxyDecision::ContinueProcessing)
    }
}
