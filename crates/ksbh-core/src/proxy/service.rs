#[allow(dead_code)]
pub struct ProxyService {
    pub(super) storage: ::std::sync::Arc<crate::Storage>,
    pub(super) sessions: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    pub(super) modules_configs_registry: crate::modules::registry::ModuleRegistryReader,
    pub(super) public_config: ksbh_types::PublicConfig,
    pub(super) config: ::std::sync::Arc<crate::Config>,
    pub(super) hosts: crate::routing::RouterReader,
    pub(super) metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
    pub(super) modules: ::std::sync::Arc<crate::modules::abi::module_host::ModuleHost>,
}

impl ProxyService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: ::std::sync::Arc<crate::Config>,
        storage: ::std::sync::Arc<crate::Storage>,
        public_config: ksbh_types::PublicConfig,
        hosts: crate::routing::RouterReader,
        modules_configs_registry: crate::modules::registry::ModuleRegistryReader,
        metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
        sessions: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
        modules: ::std::sync::Arc<crate::modules::abi::module_host::ModuleHost>,
    ) -> Self {
        Self {
            modules,
            storage,
            sessions,
            public_config,
            config: config.clone(),
            hosts,
            metrics_sender,
            modules_configs_registry,
        }
    }
}

#[async_trait::async_trait]
impl ksbh_types::prelude::ProxyProvider for ProxyService {
    type ProxyContext = crate::proxy::ProxyContext;

    fn new_context(&self) -> Self::ProxyContext {
        crate::proxy::ProxyContext::new(self.config.clone())
    }

    async fn early_request_filter(
        &self,
        _session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        _ctx: &mut Self::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        Ok(ksbh_types::prelude::ProxyDecision::ContinueProcessing)
    }

    async fn request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        Ok(self._request_filter(session, ctx).await?)
    }

    async fn upstream_peer(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> Result<ksbh_types::providers::proxy::UpstreamPeer, ksbh_types::prelude::ProxyProviderError>
    {
        use std::str::FromStr;

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
                address: self.config.listen_address_internal.to_string(),
            });
        }

        if let Some(valid_request_information) = &ctx.valid_request_information {
            return match &valid_request_information.req_match.backend {
                crate::routing::ServiceBackendType::ServiceBackend(svc) => {
                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: format!("{}:{}", svc.name, svc.port),
                    })
                }
                crate::routing::ServiceBackendType::Static => {
                    let headers = &session.headers();
                    let http_request_view =
                        match ksbh_types::requests::http_request::HttpRequestView::new(
                            headers,
                            ctx.req_id,
                            &self.public_config,
                        ) {
                            Ok(view) => view,
                            Err(_) => {
                                session.set_request_uri(
                                    http::Uri::from_str("http://internal.ksbh.rs/500")
                                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                                );

                                return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                                    address: self.config.listen_address_internal.to_string(),
                                });
                            }
                        };
                    let file_path = format!(
                        "{}/{}",
                        http_request_view.host.trim_end_matches("/"),
                        http_request_view.query
                    );
                    let file_path = urlencoding::encode(&file_path);
                    let new_path = format!("/static?file={}", file_path);

                    session.set_request_uri(
                        http::Uri::from_str(&new_path)
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: self.config.listen_address_internal.to_string(),
                    })
                }
                _ => {
                    session.set_request_uri(
                        http::Uri::from_str("http://internal.ksbh.rs/500")
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: self.config.listen_address_internal.to_string(),
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
            address: self.config.listen_address_internal.to_string(),
        });
    }

    async fn response_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        response: &mut http::response::Parts,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if ctx.proxy_decision.is_some() || session.response_sent() {
            return Ok(());
        }

        if !response.headers.contains_key(http::header::SET_COOKIE)
            && crate::cookies::ProxyCookie::from_session(session)
                .await
                .is_err()
            && let Some(valid_request_information) = &ctx.valid_request_information
        {
            let cookie = crate::cookies::ProxyCookie::new(
                valid_request_information.host.as_str(),
                None,
                valid_request_information.session_id,
            );

            response
                .headers
                .try_insert(
                    http::header::SET_COOKIE,
                    http::HeaderValue::from_str(&cookie.to_cookie_header().map_err(|e| {
                        ksbh_types::prelude::ProxyProviderError::InternalErrorDetailled(
                            e.to_string(),
                        )
                    })?)
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        response
            .headers
            .try_insert(
                crate::constants::PROXY_HEADER_NAME,
                http::HeaderValue::from_static(crate::constants::PROXY_HEADER_VALUE),
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if ctx.proxy_decision.is_some() || session.response_sent() {
            return Ok(());
        }

        let headers = &session.headers();
        let http_req = match ksbh_types::requests::http_request::HttpRequestView::new(
            headers,
            ctx.req_id,
            &self.public_config,
        ) {
            Ok(view) => view,
            Err(_) => return Ok(()),
        };

        let proto = if http_req.uri.as_str().starts_with("wss")
            || http_req.uri.as_str().starts_with("https")
        {
            "https"
        } else {
            "http"
        };
        let host_with_port = if http_req.port != 80 && http_req.port != 443 {
            format!("{}:{}", http_req.host, http_req.port)
        } else {
            http_req.host.to_string()
        };

        upstream_request
            .insert_header("X-Forwarded-Proto", http::HeaderValue::from_static(proto))
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        if proto == "https" {
            upstream_request
                .insert_header("X-Forwarded-Ssl", http::HeaderValue::from_static("on"))
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        if let Some(client_addr) = crate::utils::get_client_ip_from_session(session) {
            upstream_request
                .insert_header(
                    "X-Forwarded-For",
                    http::HeaderValue::from_str(client_addr.to_string().as_str())
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        upstream_request
            .insert_header(
                "X-Forwarded-Host",
                http::HeaderValue::from_str(host_with_port.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        upstream_request
            .insert_header(
                http::header::HOST,
                http::HeaderValue::from_str(host_with_port.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        if let Some(origin) = session.get_header(http::header::ORIGIN) {
            upstream_request
                .insert_header(http::header::ORIGIN, origin.to_owned())
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        Ok(())
    }

    async fn logging(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        _error: Option<&ksbh_types::prelude::ProxyProviderError>,
        ctx: &mut Self::ProxyContext,
    ) {
        let duration = ctx.req_start.elapsed();

        if let Some(valid_req_information) = ctx.valid_request_information.take()
            && let Some(response_header) = session.response_written()
            && let Err(e) = self
                .metrics_sender
                .send(crate::metrics::RequestMetrics::new(
                    valid_req_information,
                    ::std::mem::take(&mut ctx.modules_metrics),
                    response_header.status(),
                    duration.as_secs_f64(),
                ))
                .await
        {
            tracing::error!("There was an error sending request_metric {}", e);
        }
    }
}
