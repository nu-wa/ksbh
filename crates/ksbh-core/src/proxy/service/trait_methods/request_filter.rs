use crate::proxy::{
    PartialClientInformation, ProxyContext, RoutedRequest, ValidRequestInformation,
};
use ksbh_types::prelude::{ProxyDecision, ProxyProviderResult, ProxyProviderSession};

impl crate::proxy::ProxyService {
    pub async fn run_request_modules(
        &self,
        session: &mut dyn ProxyProviderSession,
        ctx: &mut ProxyContext,
    ) -> ProxyProviderResult {
        if ctx.proxy_decision.is_some() || session.response_sent() {
            return Ok(ksbh_types::prelude::ProxyDecision::ModuleReplied);
        }

        let observed_request = match ctx.observed_request.clone() {
            Some(observed_request) => observed_request,
            None => match self.build_observed_request(session, ctx).await {
                Ok(observed_request) => observed_request,
                Err(error) => {
                    tracing::error!("Failed to build observed request: {:?}", error);
                    let (status, body) = error.response();
                    return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                        status, body,
                    ));
                }
            },
        };

        ctx.observed_request = Some(observed_request.clone());
        ctx.http_request = Some(observed_request.http_request.clone());

        let client_information: PartialClientInformation =
            match PartialClientInformation::new_from_session(session, &self.config) {
                Some(partial_cli_info) => partial_cli_info,
                None => {
                    tracing::error!("Client has no information (user agent or ip ?)");
                    return Ok(ksbh_types::prelude::ProxyDecision::StopProcessing(
                        http::StatusCode::BAD_REQUEST,
                        bytes::Bytes::from_static(b"Bad Request"),
                    ));
                }
            };

        let http_request = &observed_request.http_request;

        let request_match = match self.hosts.find_route(http_request) {
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

        let session_id = observed_request.session_id;

        ctx.session_id_bytes = session_id.into_bytes();

        let valid_request_information = ValidRequestInformation::new(
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
        let is_websocket_handshake = observed_request.is_websocket_handshake;

        ctx.valid_request_information = Some(valid_request_information.clone());
        ctx.routed_request = Some(RoutedRequest {
            route_match: valid_request_information.req_match.clone(),
        });

        if is_websocket_handshake {
            tracing::debug!(
                "websocket handshake detected, skipping module chain for host={} path={}",
                valid_request_information.host,
                valid_request_information.path
            );
            Ok(ProxyDecision::ContinueProcessing)
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

            let outcome = self.modules.as_ref().run_chain(
                modules,
                ksbh_modules_abi::prelude::KSBHModuleStage::Request,
                &observed_request,
                session.header_map(),
                request_body.as_ref(),
                &self.config.url_paths.modules,
                ctx.needs_session_cookie,
                &self.cookie_settings,
                crate::metrics::module_metric::ModuleMetric::new_request,
                &mut ctx.modules_metrics,
            )?;

            match outcome {
                crate::modules::chain::ChainOutcome::Continue => {
                    Ok(ProxyDecision::ContinueProcessing)
                }
                crate::modules::chain::ChainOutcome::Reply(response) => {
                    session.write_response(response).await.map_err(|e| {
                        tracing::error!("Failed to write module response: {e}");
                        e
                    })?;
                    Ok(ProxyDecision::ModuleReplied)
                }
                crate::modules::chain::ChainOutcome::ServerError(body) => {
                    Ok(ProxyDecision::StopProcessing(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        body,
                    ))
                }
                crate::modules::chain::ChainOutcome::ModuleNotFound { .. } => {
                    // Chain skips NotFound modules internally;
                    // this arm is unreachable but required for exhaustiveness.
                    Ok(ProxyDecision::ContinueProcessing)
                }
            }
        }
    }
}
