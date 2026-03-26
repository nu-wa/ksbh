/// The main proxy service implementation combining routing, modules, storage, and metrics.
///
/// This struct implements the `ProxyProvider` trait from ksbh-types and orchestrates
/// the request lifecycle: request filtering, upstream peer resolution, header
/// manipulation, response filtering, and metrics collection.
#[allow(dead_code)]
pub struct ProxyService {
    #[allow(dead_code)]
    pub(super) storage: ::std::sync::Arc<crate::Storage>,
    #[allow(dead_code)]
    pub(super) sessions: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    #[allow(dead_code)]
    pub(super) modules_configs_registry: crate::modules::registry::ModuleRegistryReader,
    #[allow(dead_code)]
    pub(super) config: ::std::sync::Arc<crate::Config>,
    #[allow(dead_code)]
    pub(super) hosts: crate::routing::RouterReader,
    #[allow(dead_code)]
    pub(super) metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
    #[allow(dead_code)]
    pub(super) modules: ::std::sync::Arc<crate::modules::abi::module_host::ModuleHost>,
    pub(super) cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    pub(super) proxy_header_name: http::header::HeaderName,
    pub(super) proxy_header_value: http::HeaderValue,
}

impl ProxyService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: ::std::sync::Arc<crate::Config>,
        storage: ::std::sync::Arc<crate::Storage>,
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
        cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    ) -> Self {
        let proxy_header_name =
            http::header::HeaderName::from_bytes(config.constants.proxy_header_name.as_bytes())
                .expect("validated proxy header name must parse");
        let proxy_header_value = http::HeaderValue::from_str(&config.constants.proxy_header_value)
            .expect("validated proxy header value must parse");

        Self {
            modules,
            storage,
            sessions,
            config: config.clone(),
            hosts,
            metrics_sender,
            modules_configs_registry,
            cookie_settings,
            proxy_header_name,
            proxy_header_value,
        }
    }
}

#[async_trait::async_trait]
impl ksbh_types::prelude::ProxyProvider for ProxyService {
    type ProxyContext = crate::proxy::ProxyContext;

    fn new_context(&self) -> Self::ProxyContext {
        crate::proxy::ProxyContext::new(self.config.clone())
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
                    let http_request = match &ctx.http_request {
                        Some(req) => req,
                        None => {
                            session.set_request_uri(
                                http::Uri::from_str("http://internal.ksbh.rs/500")
                                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                            );

                            return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                                address: internal_upstream_address.clone(),
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
                    })
                }
                _ => {
                    session.set_request_uri(
                        http::Uri::from_str("http://internal.ksbh.rs/500")
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: internal_upstream_address.clone(),
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
            && ctx.needs_session_cookie
            && let Some(valid_request_information) = &ctx.valid_request_information
        {
            let cookie = crate::cookies::ProxyCookie::new(
                valid_request_information.host.as_str(),
                valid_request_information.session_id,
            );

            response
                .headers
                .try_insert(
                    http::header::SET_COOKIE,
                    http::HeaderValue::from_str(
                        &cookie
                            .to_cookie_header(&self.cookie_settings)
                            .map_err(|e| {
                                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                                    e.to_string(),
                                )
                            })?,
                    )
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        if ctx.downstream_ws_kind != crate::proxy::DownstreamWebsocketKind::None {
            response
                .headers
                .try_insert(
                    crate::constants::HEADER_X_KSBH_WS_DOWNSTREAM_TRANSPORT,
                    http::HeaderValue::from_str(ctx.downstream_transport.as_str())
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        response
            .headers
            .try_insert(
                self.proxy_header_name.clone(),
                self.proxy_header_value.clone(),
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        upstream_request: &mut pingora_http::RequestHeader,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if ctx.proxy_decision.is_some() || session.response_sent() {
            return Ok(());
        }

        let http_req = match &ctx.http_request {
            Some(req) => req,
            None => return Ok(()),
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
            .insert_header(
                crate::constants::HEADER_X_FORWARDED_PROTO,
                http::HeaderValue::from_static(proto),
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        if proto == "https" {
            upstream_request
                .insert_header(
                    crate::constants::HEADER_X_FORWARDED_SSL,
                    http::HeaderValue::from_static("on"),
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        if let Some(client_addr) = crate::utils::get_client_ip_from_session(session) {
            upstream_request
                .insert_header(
                    crate::constants::HEADER_X_FORWARDED_FOR,
                    http::HeaderValue::from_str(client_addr.to_string().as_str())
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        upstream_request
            .insert_header(
                crate::constants::HEADER_X_FORWARDED_HOST,
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
