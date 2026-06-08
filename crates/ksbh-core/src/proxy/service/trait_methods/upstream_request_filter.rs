use ksbh_types::prelude::ProxyProviderError;

impl crate::proxy::ProxyService {
    pub async fn upstream_request_filter_impl(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        upstream_request: &mut pingora_http::RequestHeader,
        ctx: &mut crate::proxy::ProxyContext,
    ) -> Result<(), ProxyProviderError> {
        if ctx.proxy_decision.is_some() || session.response_sent() {
            return Ok(());
        }

        Self::normalize_cookie_header_for_upstream(upstream_request)?;

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

        let trust_forwarded_headers = self
            .config
            .trusts_forwarded_headers_from(session.client_addr());
        let direct_client_ip = session.client_addr();
        let effective_client_ip =
            crate::utils::get_client_ip_from_session(session, trust_forwarded_headers);

        if let Some(effective_client_ip) = effective_client_ip {
            upstream_request
                .insert_header(
                    crate::constants::HEADER_X_REAL_IP,
                    http::HeaderValue::from_str(effective_client_ip.to_string().as_str())
                        .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        if let Some(client_addr) = direct_client_ip.or(effective_client_ip) {
            let forwarded_for = Self::compose_forwarded_header_value(
                session
                    .header_map()
                    .get(crate::constants::HEADER_X_FORWARDED_FOR),
                client_addr.to_string().as_str(),
                trust_forwarded_headers,
            );
            upstream_request
                .insert_header(
                    crate::constants::HEADER_X_FORWARDED_FOR,
                    http::HeaderValue::from_str(forwarded_for.as_str())
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
                crate::constants::HEADER_X_FORWARDED_PORT,
                http::HeaderValue::from_str(http_req.port.to_string().as_str())
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
        let forwarded_entry = Self::forwarded_header_entry(
            direct_client_ip.or(effective_client_ip),
            proto,
            &host_with_port,
        );
        let forwarded_value = Self::compose_forwarded_header_value(
            session.header_map().get(crate::constants::HEADER_FORWARDED),
            &forwarded_entry,
            trust_forwarded_headers,
        );
        upstream_request
            .insert_header(
                crate::constants::HEADER_FORWARDED,
                http::HeaderValue::from_str(forwarded_value.as_str())
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
}
