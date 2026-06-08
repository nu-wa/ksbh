impl crate::proxy::ProxyService {
    pub async fn logging_impl(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        _error: Option<&ksbh_types::prelude::ProxyProviderError>,
        ctx: &mut crate::proxy::ProxyContext,
    ) {
        let duration = ctx.req_start.elapsed();
        let response_status = session.response_status();
        let mut modules_metrics = ::std::mem::take(&mut ctx.modules_metrics);
        let logging_modules = if let Some(valid_request_information) =
            ctx.valid_request_information.as_ref()
        {
            valid_request_information
                .req_match
                .modules
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        } else {
            self.hosts.get_global_modules_configs()
        };

        if ctx.needs_completion_signal {
            crate::metrics::runtime_signals::RUNTIME_SIGNALS.request_finished();
            ctx.needs_completion_signal = false;
        }

        if let Some(response_status) = response_status {
            crate::metrics::runtime_signals::RUNTIME_SIGNALS.observe_completion(response_status);
        }

        if let (Some(observed_request), Some(response_status)) =
            (ctx.observed_request.as_ref(), response_status)
        {
            let reputation_delta = crate::metrics::RequestMetrics::calculate_score_parts(
                response_status,
                duration.as_secs_f64(),
                &modules_metrics,
            )
            .max(0) as u64;

            self.modules.reputation_observe(
                observed_request.client.reputation_key,
                Some(observed_request.client.ip),
                reputation_delta,
            );
        }

        if let Some(observed_request) = ctx.observed_request.as_ref() {
            for module in logging_modules {
                let start = ::std::time::Instant::now();

                let mod_call_result =
                    self.modules
                        .call_module(crate::modules::runtime::ModuleCallInput {
                            stage: ksbh_modules_abi::prelude::KSBHModuleStage::Logging,
                            module_name: module.name.as_str(),
                            module_type: module.mod_spec.r#type.clone(),
                            config: &module.config_values,
                            observed: observed_request,
                            headers: session.header_map(),
                            body: ctx.buffered_request_body.as_ref(),
                            internal_path: self.config.url_paths.modules.as_str(),
                            needs_session_cookie: ctx.needs_session_cookie,
                        });

                let mod_exec_time = start.elapsed().as_secs_f64();

                match mod_call_result {
                    Err(e) => {
                        tracing::error!("Module {} error during logging: {:?}", module.name, e);
                        modules_metrics.push(
                            crate::metrics::module_metric::ModuleMetric::new_request(
                                module.name.as_str(),
                                mod_exec_time,
                                true,
                                true,
                            ),
                        );
                    }
                    Ok(crate::modules::runtime::ModuleCallOutcome::Pass) => {
                        tracing::debug!("Module {} logging executed successfully", module.name);
                        modules_metrics.push(
                            crate::metrics::module_metric::ModuleMetric::new_request(
                                module.name.as_str(),
                                mod_exec_time,
                                true,
                                false,
                            ),
                        );
                    }
                    Ok(crate::modules::runtime::ModuleCallOutcome::Stop(response)) => {
                        tracing::debug!(
                            "Module {} returned a response during logging, ignoring status {}",
                            module.name,
                            response.status()
                        );
                        modules_metrics.push(
                            crate::metrics::module_metric::ModuleMetric::new_request(
                                module.name.as_str(),
                                mod_exec_time,
                                true,
                                true,
                            ),
                        );
                    }
                    Ok(crate::modules::runtime::ModuleCallOutcome::Error(message)) => {
                        tracing::error!(
                            "Module {} returned error during logging: {}",
                            module.name,
                            message
                        );
                        modules_metrics.push(
                            crate::metrics::module_metric::ModuleMetric::new_request(
                                module.name.as_str(),
                                mod_exec_time,
                                true,
                                true,
                            ),
                        );
                    }
                }
            }
        }

        if let Some(valid_req_information) = ctx.valid_request_information.take()
            && let Some(response_status) = response_status
            && let Err(e) = self
                .metrics_sender
                .send(crate::metrics::RequestMetrics::new(
                    valid_req_information,
                    modules_metrics,
                    response_status,
                    duration.as_secs_f64(),
                ))
                .await
        {
            tracing::error!("There was an error sending request_metric {}", e);
        }
    }
}
