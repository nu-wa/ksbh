impl super::ProxyService {
    pub(super) async fn _early_request_filter(
        &self,
        session: &mut pingora::proxy::Session,
        ctx: &mut ksbh_core::proxy::ProxyContext,
    ) -> pingora::prelude::Result<()> {
        let client_information: ksbh_core::proxy::PartialClientInformation =
            match ksbh_core::proxy::ClientInformation::new_from_session(session) {
                Some(cli_info) => cli_info.into(),
                None => match ksbh_core::proxy::PartialClientInformation::new_from_session(session)
                {
                    Some(partial_cli_info) => partial_cli_info,
                    None => {
                        tracing::error!("Client has no information (user agent or ip ?)");
                        ctx.already_replied = true;
                        session.respond_error(400).await?;
                        return Ok(());
                    }
                },
            };
        let req_id = uuid::Uuid::new_v4();

        let http_request_info = match ksbh_types::prelude::HttpRequest::new(
            session.req_header(),
            req_id,
            &self.public_config,
        ) {
            Ok(h) => {
                ctx.partial_request_information =
                    Some(ksbh_core::proxy::PartialRequestInformation {
                        http_request_info: h.clone(),
                        client_information: client_information.clone(),
                    });
                h
            }
            Err(e) => {
                tracing::error!("{e}");
                ctx.already_replied = true;
                session.respond_error(400).await?;
                return Ok(());
            }
        };

        let (proxy_cookie, had_cookie) =
            match ksbh_core::cookies::ProxyCookie::from_pingora_session(session) {
                Ok(cookie) => (cookie, true),
                Err(e) => match e {
                    ksbh_core::cookies::ProxyCookieError::NoCookie => (
                        ksbh_core::cookies::ProxyCookie::new(
                            http_request_info.host.as_str(),
                            None,
                            uuid::Uuid::new_v4(),
                        ),
                        false,
                    ),

                    _ => {
                        tracing::error!("{e}");
                        ctx.already_replied = true;
                        session.respond_error(400).await?;
                        return Ok(());
                    }
                },
            };

        ctx.had_cookie = had_cookie;

        let mut early_request_info = ksbh_core::proxy::EarlyRequestInformation {
            session: ksbh_core::proxy::ProxySession {
                id: proxy_cookie.session_id,
            },
            cookie: proxy_cookie,
            http_request_info: http_request_info.clone(),
            config: ctx.config.clone(),
            client_information,
        };

        let req_match = self.hosts.match_req(&http_request_info);

        // Let's not bother with anything apart from logging if the request has no matching route.
        // TODO: Render html ?
        if req_match.is_none() {
            ctx.already_replied = true;
            session.respond_error(400).await?;
            return Ok(());
        }

        let modules_metrics = &mut ctx.modules_metrics;
        let mut replied = false;

        for (mod_name, mod_type, mod_cfg) in self.modules.get_global_configs() {
            if let Some(module) = crate::get_module_by_type(mod_type) {
                let start = ::std::time::Instant::now();

                let mod_call_result = module
                    .early_request_filter(&mod_cfg, session, &mut early_request_info, &self.storage)
                    .await;

                let mod_exec_time = start.elapsed().as_secs_f64();

                if let Err(e) = &mod_call_result {
                    replied = true;
                    let mod_error = e.early_to_pingora().await;
                    session
                        .respond_error_with_body(mod_error.0.as_u16(), mod_error.1)
                        .await?;
                }

                if let Ok(mod_has_replied) = mod_call_result
                    && mod_has_replied
                {
                    replied = true;
                }

                modules_metrics.push(ksbh_core::metrics::module_metric::ModuleMetric::new_early(
                    mod_name.as_str(),
                    mod_exec_time,
                    true,
                    replied,
                ));

                if replied {
                    break;
                }
            } else {
                tracing::warn!("Tried to call {mod_name} but it's not configured.");
            }
        }

        if replied {
            ctx.already_replied = true;
            return Ok(());
        }

        ctx.early_request_information = Some(early_request_info.clone());

        // If we are here we already checked if req_match.is_none().
        let req_match = req_match.unwrap();

        ctx.backend = req_match.backend.clone();

        for module_cfg_name in req_match.host.modules.iter() {
            match self.modules.get_config(module_cfg_name.as_str()) {
                Some((mod_type, mod_cfg)) => {
                    if let Some(module) = crate::get_module_by_type(mod_type) {
                        let start = ::std::time::Instant::now();

                        let mod_call_result = module
                            .early_request_filter(
                                &mod_cfg,
                                session,
                                &mut early_request_info,
                                &self.storage,
                            )
                            .await;

                        let mod_exec_time = start.elapsed().as_secs_f64();

                        if let Err(e) = &mod_call_result {
                            replied = true;
                            let mod_error = e.early_to_pingora().await;
                            session
                                .respond_error_with_body(mod_error.0.as_u16(), mod_error.1)
                                .await?;
                        }

                        if let Ok(mod_has_replied) = mod_call_result
                            && mod_has_replied
                        {
                            replied = true;
                        }
                        modules_metrics.push(
                            ksbh_core::metrics::module_metric::ModuleMetric::new_early(
                                module_cfg_name.as_str(),
                                mod_exec_time,
                                false,
                                replied,
                            ),
                        );

                        if replied {
                            break;
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        "Calling module '{}' without a configuration",
                        module_cfg_name
                    );
                }
            }
        }

        if replied {
            ctx.already_replied = true;
            return Ok(());
        }

        ctx.valid_request_information =
            Some(ksbh_core::proxy::ValidRequestInformation::new_from_early(
                early_request_info,
                req_match,
            ));

        Ok(())
    }
}
