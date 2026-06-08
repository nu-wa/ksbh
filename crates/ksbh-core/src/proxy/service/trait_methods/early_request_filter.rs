use crate::proxy::ProxyContext;
use ksbh_types::prelude::{ProxyDecision, ProxyProviderResult, ProxyProviderSession};

impl crate::proxy::ProxyService {
    pub async fn run_early_modules(
        &self,
        session: &mut dyn ProxyProviderSession,
        ctx: &mut ProxyContext,
    ) -> ProxyProviderResult {
        match self.build_observed_request(session, ctx).await {
            Ok(observed_request) => {
                ctx.observed_request = Some(observed_request.clone());
                ctx.http_request = Some(observed_request.http_request.clone());

                let outcome = self.modules.as_ref().run_chain(
                    &self.hosts.get_global_modules_configs(),
                    ksbh_modules_abi::prelude::KSBHModuleStage::BeforeRouting,
                    &observed_request,
                    session.header_map(),
                    None,
                    &self.config.url_paths.modules,
                    ctx.needs_session_cookie,
                    &self.cookie_settings,
                    crate::metrics::module_metric::ModuleMetric::new_early,
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
            Err(error) => {
                tracing::error!("Failed to build observed request: {:?}", error);
                let (status, body) = error.response();
                Ok(ProxyDecision::StopProcessing(status, body))
            }
        }
    }
}
