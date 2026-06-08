impl crate::proxy::ProxyService {
    pub async fn resolve_upstream_peer(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<ksbh_types::providers::proxy::UpstreamPeer, ksbh_types::prelude::ProxyProviderError>
    {
        use std::str::FromStr;
        let internal_upstream_address = self
            .config
            .listen_addresses
            .internal_connect_addr()
            .to_string();

        // Return to internal error page
        if let Some(ksbh_types::prelude::ProxyDecision::StopProcessing(
            decision_code,
            _decision_msg,
        )) = &ctx.proxy_decision
        {
            let uri = ::std::format!("http://internal.ksbh.rs/{}", decision_code.as_str());

            session.set_request_uri(
                http::Uri::from_str(uri.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            );

            return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                address: internal_upstream_address.clone(),
                peer_options: None,
            });
        }

        if let Some(valid_request_information) = &ctx.valid_request_information {
            let req_match = &valid_request_information.req_match;
            return match &req_match.destination {
                crate::routing::RoutingDestination::Upstream(svc) => {
                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: format!("{}:{}", svc.name, svc.port),
                        peer_options: req_match.peer_options.clone(),
                    })
                }
                crate::routing::RoutingDestination::Static => {
                    let http_request = match &ctx.http_request {
                        Some(req) => req,
                        None => {
                            session.set_request_uri(
                                http::Uri::from_str("http://internal.ksbh.rs/500")
                                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                            );

                            return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                                address: internal_upstream_address.clone(),
                                peer_options: None,
                            });
                        }
                    };
                    let request_path = urlencoding::encode(&http_request.query.path);
                    let new_path = format!("/static?path={request_path}");

                    session.set_request_uri(
                        http::Uri::from_str(&new_path)
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: internal_upstream_address.clone(),
                        peer_options: None,
                    })
                }
                _ => {
                    session.set_request_uri(
                        http::Uri::from_str("http://internal.ksbh.rs/500")
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: internal_upstream_address.clone(),
                        peer_options: None,
                    })
                }
            };
        }

        // No request match, no module replied, or invalid request
        session.set_request_uri(
            http::Uri::from_str("http://internal.ksbh.rs/404")
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
        );

        return Ok(ksbh_types::providers::proxy::UpstreamPeer {
            address: internal_upstream_address,
            peer_options: None,
        });
    }
}
