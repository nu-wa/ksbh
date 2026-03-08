impl super::ProxyService {
    pub(super) async fn _request_filter(
        &self,
        session: &mut pingora::proxy::Session,
        ctx: &mut ksbh_core::proxy::ProxyContext,
    ) -> pingora::prelude::Result<bool> {
        if ctx.valid_request_information.is_none() {
            ctx.already_replied = true;
            session.respond_error(400).await?;
            return Ok(true);
        }

        if ctx.already_replied {
            return Ok(true);
        }

        let mut replied = false;
        let modules_metrics = &mut ctx.modules_metrics;

        let mut valid_req_info = ctx.valid_request_information.take().unwrap();

        for (mod_name, mod_type, mod_cfg) in self.modules.get_global_configs() {
            if let Some(module) = crate::get_module_by_type(mod_type) {
                let start = ::std::time::Instant::now();

                let mod_call_result = module
                    .proxy_request_filter(&mod_cfg, session, &mut valid_req_info, &self.storage)
                    .await;

                let mod_exec_time = start.elapsed().as_secs_f64();

                if let Err(e) = &mod_call_result {
                    replied = true;
                    let _ = e.to_pingora(session).await?;
                }

                if let Ok(mod_has_replied) = mod_call_result
                    && mod_has_replied
                {
                    replied = true;
                }

                modules_metrics.push(
                    ksbh_core::metrics::module_metric::ModuleMetric::new_request(
                        mod_name.as_str(),
                        mod_exec_time,
                        true,
                        replied,
                    ),
                );

                if replied {
                    break;
                }
            } else {
                tracing::warn!("Calling {mod_name} but it's not configured.");
            }
        }

        if replied {
            ctx.already_replied = true;
            return Ok(true);
        }

        let req_match = &valid_req_info.req_match.clone();

        for module_cfg_name in &req_match.host.modules {
            match self.modules.get_config(module_cfg_name.as_str()) {
                Some((mod_type, mod_cfg)) => {
                    if let Some(module) = crate::get_module_by_type(mod_type) {
                        let start = ::std::time::Instant::now();

                        let mod_call_result = module
                            .proxy_request_filter(
                                &mod_cfg,
                                session,
                                &mut valid_req_info,
                                &self.storage,
                            )
                            .await;

                        let mod_exec_time = start.elapsed().as_secs_f64();

                        if let Err(e) = &mod_call_result {
                            replied = true;
                            let _ = e.to_pingora(session).await?;
                        }

                        if let Ok(mod_has_replied) = mod_call_result
                            && mod_has_replied
                        {
                            replied = true;
                        }
                        modules_metrics.push(
                            ksbh_core::metrics::module_metric::ModuleMetric::new_request(
                                module_cfg_name.as_str(),
                                mod_exec_time,
                                true,
                                replied,
                            ),
                        );

                        if replied {
                            break;
                        }
                    }
                }
                None => {
                    tracing::warn!("Calling module {module_cfg_name} without a configuration");
                }
            }
        }

        if replied {
            ctx.already_replied = true;
            return Ok(true);
        }

        /* let plugin_metrics = &mut ctx.plugins_metrics;

        for enabled_plugin_name in valid_req_info.req_match.host.plugins.iter() {
            let start = ::std::time::Instant::now();
            let plugin_call_result = ksbh_core::plugin::call_plugin(
                extism_plugin_cache.clone(),
                enabled_plugin_name,
                self.public_config,
                &valid_req_info.http_request_info,
            )
            .await;
            let plugin_exec_time = start.elapsed().as_secs_f64();

            if let ksbh_types::prelude::PluginOutput::Replied(plugin_response) = plugin_call_result
            {
                if plugin_response.changed {
                    let mut response_headers = pingora::http::ResponseHeader::build(
                        match pingora::http::StatusCode::from_u16(plugin_response.status_code) {
                            Ok(code) => code,
                            Err(_) => {
                                ctx.already_replied = true;
                                return Err(pingora::Error::new(pingora::ErrorType::UnknownError));
                            }
                        },
                        None,
                    )?;

                    for header in plugin_response.headers {
                        response_headers
                            .insert_header(header.0.to_string(), header.1.to_string())?;
                    }

                    session
                        .write_response_header(Box::new(response_headers), true)
                        .await?;
                } else {
                    let req_header = session.req_header_mut();
                    for header in plugin_response.headers {
                        req_header.insert_header(header.0.to_string(), header.1.to_string())?;
                    }
                }
            }

            plugin_metrics.push(ksbh_core::metrics::plugin_metric::PluginMetric::new(
                enabled_plugin_name.as_str(),
                plugin_exec_time,
                replied,
            ));

            if replied {
                break;
            }
        }*/

        if replied {
            ctx.already_replied = true;
            return Ok(true);
        }

        ctx.valid_request_information = Some(valid_req_info);

        Ok(false)
    }
}
