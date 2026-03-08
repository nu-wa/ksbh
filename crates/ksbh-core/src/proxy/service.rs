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
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        let proxy_decision = self._early_request_filter(session, ctx).await?;

        ctx.proxy_decision = Some(proxy_decision.clone());

        Ok(proxy_decision)
    }

    async fn request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        if ctx.proxy_decision.is_some() {
            return Ok(ksbh_types::prelude::ProxyDecision::ContinueProcessing);
        }

        let proxy_decision = self._request_filter(session, ctx).await?;

        ctx.proxy_decision = Some(proxy_decision.clone());

        Ok(proxy_decision)
    }

    async fn upstream_peer(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> Result<ksbh_types::providers::proxy::UpstreamPeer, ksbh_types::prelude::ProxyProviderError>
    {
        use std::str::FromStr;

        if let Some(proxy_decision) = &ctx.proxy_decision
            && *proxy_decision != ksbh_types::prelude::ProxyDecision::ContinueProcessing
        {
            match proxy_decision {
                ksbh_types::prelude::ProxyDecision::ModuleReplied => {
                    tracing::debug!("ModuleReplied: response already sent, not calling upstream");
                    return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: self.config.listen_address_internal.to_string(),
                    });
                }
                ksbh_types::prelude::ProxyDecision::StopProcessing(http_code, body) => {
                    tracing::debug!(
                        "StopProcessing: {http_code} body: {:?}",
                        String::from_utf8(body.to_vec())
                    );

                    let error_code = match *http_code {
                        http::StatusCode::NOT_FOUND => "404",
                        http::StatusCode::BAD_REQUEST => "400",
                        _ => "500",
                    };

                    let uri = format!("http://internal.ksbh.rs/{error_code}");
                    session.set_request_uri(
                        http::Uri::from_str(uri.as_str())
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    tracing::debug!("Proxy Decision: {:?}, uri: {:?}", proxy_decision, uri);

                    return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: self.config.listen_address_internal.to_string(),
                    });
                }
                _ => {
                    return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: self.config.listen_address_internal.to_string(),
                    });
                }
            };
        };

        match &ctx.backend {
            crate::routing::ServiceBackendType::ServiceBackend(svc) => {
                Ok(ksbh_types::providers::proxy::UpstreamPeer {
                    address: format!("{}:{}", svc.name, svc.port),
                })
            }
            crate::routing::ServiceBackendType::ToSelf(_prefix) => {
                Ok(ksbh_types::providers::proxy::UpstreamPeer {
                    address: self.config.listen_address_api.to_string(),
                })
            }
            crate::routing::ServiceBackendType::Static => {
                let http_req = match &ctx.partial_request_information {
                    Some(partial_req_info) => &partial_req_info.http_request_info,
                    None => {
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
                    http_req.host.as_str().trim_end_matches("/"),
                    http_req.query
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
            crate::routing::ServiceBackendType::None => {
                session.set_request_uri(
                    http::Uri::from_str("http://internal.ksbh.rs/404")
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                );

                return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                    address: self.config.listen_address_internal.to_string(),
                });
            }
            crate::routing::ServiceBackendType::Error(name) => {
                let uri = format!("http://internal.ksbh.rs/{}", name);
                session.set_request_uri(
                    http::Uri::from_str(uri.as_str())
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                );

                return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                    address: self.config.listen_address_internal.to_string(),
                });
            }
        }
    }

    async fn response_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        response: &mut http::response::Parts,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if ctx.proxy_decision.is_some() {
            return Ok(());
        }

        if let Some(valid_req_info) = &ctx.valid_request_information
            && session.get_header(http::header::UPGRADE).is_none()
        {
            let module_already_set_cookie =
                response.headers.get(http::header::SET_COOKIE).is_some();

            if !module_already_set_cookie {
                match valid_req_info.cookie.to_cookie_header() {
                    Ok(header_string) => {
                        tracing::debug!(
                            "Setting cookie in response (no module cookie): {}",
                            header_string
                        );
                        response
                            .headers
                            .try_insert(
                                http::header::SET_COOKIE,
                                http::HeaderValue::from_str(header_string.as_str())
                                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                            )
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
                    }
                    Err(e) => {
                        tracing::error!("Failed to set cookie: {e}");
                    }
                }
            } else {
                tracing::debug!("Module already set cookie, skipping");
            }
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
        if ctx.proxy_decision.is_some() {
            return Ok(());
        }

        let http_req = match &ctx.valid_request_information {
            Some(v) => &v.http_request_info,
            None => match &ctx.early_request_information {
                Some(e) => &e.http_request_info,
                None => return Ok(()),
            },
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

        let client_information = if let Some(valid_req_info) = ctx.valid_request_information.take()
        {
            Some((
                valid_req_info.http_request_info,
                valid_req_info.client_information,
            ))
        } else if let Some(partial_req_info) = ctx.partial_request_information.take() {
            Some((
                partial_req_info.http_request_info,
                partial_req_info.client_information,
            ))
        } else {
            ctx.early_request_information.take().map(|early_req_info| {
                (
                    early_req_info.http_request_info,
                    early_req_info.client_information,
                )
            })
        };

        if let Some(client_information) = client_information
            && let Some(response_header) = session.response_written()
            && let Err(e) = self
                .metrics_sender
                .send(crate::metrics::RequestMetrics::new(
                    client_information.1,
                    client_information.0,
                    ::std::mem::replace(&mut ctx.backend, crate::routing::ServiceBackendType::None),
                    true,
                    response_header.status(),
                    ::std::mem::take(&mut ctx.modules_metrics),
                    duration.as_secs_f64(),
                ))
                .await
        {
            tracing::error!("There was an error sending request_metric {}", e);
        }
    }
}
