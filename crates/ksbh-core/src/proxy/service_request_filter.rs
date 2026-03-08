impl super::ProxyService {
    pub(super) async fn _request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        let valid_request_info = match &ctx.valid_request_information {
            Some(v) => v,
            None => {
                tracing::debug!("No valid request information, skipping");
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::BAD_REQUEST,
                    bytes::Bytes::from_static(b"No valid request information"),
                ));
            }
        };

        let modules = &valid_request_info.req_match.modules;

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

        let headers = session.headers();
        let req_id = uuid::Uuid::new_v4();

        let http_request_view = match ksbh_types::requests::http_request::HttpRequestView::new(
            &headers,
            req_id,
            &self.public_config,
        ) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to create request view: {}", e);
                return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                    http::StatusCode::BAD_REQUEST,
                    bytes::Bytes::from_static(b"Bad Request"),
                ));
            }
        };

        let modules_metrics = &mut ctx.modules_metrics;

        for module in modules.iter() {
            let start = ::std::time::Instant::now();

            let mod_call_result = self.modules.call_module(
                &module.mod_spec.r#type,
                &module.config_kv_slice,
                session,
                &http_request_view,
                request_body.as_deref(),
                module.name.as_str(),
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
                tracing::debug!("Module {} wrote a response, stopping module chain", module.name);
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

        Ok(ksbh_types::prelude::ProxyDecision::ContinueProcessing)
    }
}
