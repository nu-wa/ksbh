impl super::ProxyService {
    pub(super) async fn _request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        let client_information: crate::proxy::PartialClientInformation =
            match crate::proxy::ClientInformation::new_from_session(session, &self.config) {
                Some(cli_info) => cli_info.into(),
                None => match crate::proxy::PartialClientInformation::new_from_session(
                    session,
                    &self.config,
                ) {
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

        let req_id = ctx.req_id;
        let headers = &session.headers();
        let trust_forwarded_headers = self
            .config
            .trusts_forwarded_headers_from(session.client_addr());
        let downstream_tls = session
            .server_addr()
            .map(|addr| addr.port() == self.config.listen_addresses.https.port())
            .unwrap_or(false);

        let http_request = match ksbh_types::requests::http_request::HttpRequest::new(
            headers,
            req_id,
            &self.config.ports.external,
            downstream_tls,
            trust_forwarded_headers,
        ) {
            Ok(req) => req,
            Err(e) => {
                tracing::error!("Failed to create HttpRequest: {:?}", e);
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    bytes::Bytes::from_static(b"Internal Server Error"),
                ));
            }
        };

        let request_match = match self.hosts.find_route(&http_request) {
            Some(req_match) => req_match,
            None => {
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::NOT_FOUND,
                    bytes::Bytes::from_static(b"Not Found"),
                ));
            }
        };

        if ctx.parsed_cookie.is_none() {
            ctx.parsed_cookie =
                crate::cookies::ProxyCookie::from_session(&self.cookie_settings, session)
                    .await
                    .ok();
        }

        ctx.needs_session_cookie = ctx.parsed_cookie.is_none();

        let session_id = ctx
            .parsed_cookie
            .as_ref()
            .map(|c| c.session_id)
            .unwrap_or_else(uuid::Uuid::new_v4);

        ctx.session_id_bytes = session_id.into_bytes();

        let valid_request_information = super::ValidRequestInformation::new(
            http_request.scheme.clone(),
            smol_str::SmolStr::new(http_request.host.as_str()),
            ksbh_types::KsbhStr::new(http_request.query.path.as_str()),
            http_request.method.clone(),
            client_information.clone(),
            self.config.clone(),
            request_match,
            session_id,
        );
        let modules = &valid_request_information.req_match.modules;
        ctx.metrics_key = ctx.session_id_bytes.to_vec();
        let is_websocket_handshake =
            ctx.downstream_ws_kind != crate::proxy::DownstreamWebsocketKind::None;

        ctx.http_request = Some(http_request.clone());
        ctx.valid_request_information = Some(valid_request_information.clone());

        if is_websocket_handshake {
            tracing::debug!(
                "websocket handshake detected, skipping module chain for host={} path={}",
                valid_request_information.host,
                valid_request_information.path
            );
        } else {
            let requires_body = modules.iter().any(|m| m.mod_spec.requires_body);
            let request_body = if requires_body {
                match session.read_request_body().await {
                    Ok(Some(body)) => Some(body),
                    Ok(None) => None,
                    Err(e) => {
                        tracing::error!("Failed to read request body: {e}");
                        return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                            http::StatusCode::BAD_REQUEST,
                            bytes::Bytes::from_static(b"Bad Request"),
                        ));
                    }
                }
            } else {
                None
            };

            ctx.buffered_request_body = request_body.clone();

            tracing::debug!(
                "request_body: {:?}, requires_body: {:?}, modules: {:?}",
                request_body,
                requires_body,
                modules
            );

            let modules_metrics = &mut ctx.modules_metrics;
            let internal_path = self.config.url_paths.modules.as_str();
            let req_ctx = crate::modules::abi::ModuleRequestContext::new(
                session,
                &http_request,
                is_websocket_handshake,
                ctx.needs_session_cookie,
                ctx.session_id_bytes,
                &ctx.metrics_key,
                request_body,
                internal_path,
            );

            for module in modules.iter() {
                let start = ::std::time::Instant::now();

                let params = crate::modules::abi::ModuleCallParams::new(
                    module.mod_spec.r#type.clone(),
                    module.config_kv_slice.clone(),
                    module.name.clone(),
                );

                let mod_call_result = self.modules.call_module(&req_ctx, &params, session).await;

                let mod_exec_time = start.elapsed().as_secs_f64();

                match mod_call_result {
                    Err(e) => {
                        tracing::error!("Module {} error: {:?}", module.name, e);
                    }
                    Ok(()) => {
                        tracing::debug!("Module {} executed successfully", module.name);
                    }
                }

                let decision = if session.response_written() {
                    tracing::debug!(
                        "Module {} wrote a response, stopping module chain",
                        module.name
                    );
                    Some(ksbh_types::prelude::ProxyDecision::ModuleReplied)
                } else {
                    None
                };

                modules_metrics.push(crate::metrics::module_metric::ModuleMetric::new_request(
                    module.name.as_str(),
                    mod_exec_time,
                    true,
                    decision.is_some(),
                ));

                if decision.is_some() {
                    return Ok(ksbh_types::prelude::ProxyDecision::ModuleReplied);
                }
            }
        }

        if ctx.downstream_ws_kind == crate::proxy::DownstreamWebsocketKind::H2ExtendedConnect
            && let crate::routing::ServiceBackendType::ServiceBackend(svc) =
                &valid_request_information.req_match.backend
        {
            ctx.tunnel_plan = Some(crate::proxy::WebsocketTunnelPlan {
                upstream_addr: format!("{}:{}", svc.name, svc.port),
                host: valid_request_information.host.clone(),
                path_and_query: ksbh_types::KsbhStr::new(http_request.query.to_string()),
            });
        }

        Ok(ksbh_types::prelude::ProxyDecision::ContinueProcessing)
    }
}
