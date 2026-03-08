impl super::ProxyService {
    pub(super) async fn _request_filter(
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
        let headers = &session.headers();
        let http_request_information =
            match ksbh_types::requests::http_request::HttpRequestView::new(
                headers,
                req_id,
                &self.public_config,
            ) {
                Ok(view) => view,
                Err(e) => {
                    tracing::error!("Failed to create HttpRequestView: {:?}", e);
                    return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        bytes::Bytes::from_static(b"Internal Server Error"),
                    ));
                }
            };
        let request_match = match self.hosts.find_route(&http_request_information) {
            Some(req_match) => req_match,
            None => {
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::NOT_FOUND,
                    bytes::Bytes::from_static(b"Not Found"),
                ));
            }
        };

        let session_id = match crate::cookies::ProxyCookie::from_session(session).await {
            Ok(cookie) => cookie.session_id,
            Err(_) => uuid::Uuid::new_v4(),
        };

        let valid_request_information = super::ValidRequestInformation::new(
            smol_str::SmolStr::new(http_request_information.host),
            client_information.clone(),
            self.config.clone(),
            request_match,
            session_id,
        );
        let modules = &valid_request_information.req_match.modules;
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

        tracing::debug!(
            "request_body: {:?}, requires_body: {:?}, modules: {:?}",
            request_body,
            requires_body,
            modules
        );

        let modules_metrics = &mut ctx.modules_metrics;

        for module in modules.iter() {
            let start = ::std::time::Instant::now();

            let mod_call_result = self.modules.call_module(
                &module.mod_spec.r#type,
                &module.config_kv_slice,
                session,
                &http_request_information,
                request_body.as_deref(),
                module.name.as_str(),
                valid_request_information.session_id,
            );

            let mod_exec_time = start.elapsed().as_secs_f64();

            match mod_call_result.await {
                Err(e) => {
                    tracing::error!("Module {} error: {:?}", module.name, e);
                }
                Ok(()) => {
                    tracing::debug!("Module {} executed successfully", module.name);
                }
            }

            let decision = if session.response_written().is_some() {
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
                decision.clone(),
            ));

            if decision.is_some() {
                return Ok(ksbh_types::prelude::ProxyDecision::ModuleReplied);
            }
        }

        ctx.valid_request_information = Some(valid_request_information);

        Ok(ksbh_types::prelude::ProxyDecision::ContinueProcessing)
    }
}
