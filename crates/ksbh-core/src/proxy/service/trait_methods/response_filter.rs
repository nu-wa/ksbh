use crate::proxy::ProxyContext;

impl crate::proxy::ProxyService {
    pub async fn apply_response_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        response: &mut http::response::Parts,
        ctx: &mut ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if ctx.proxy_decision.is_some() || session.response_sent() {
            return Ok(());
        }

        if !response.headers.contains_key(http::header::SET_COOKIE)
            && ctx.needs_session_cookie
            && let Some(valid_request_information) = &ctx.valid_request_information
        {
            let cookie = crate::cookies::ProxyCookie::new(
                valid_request_information.host.as_str(),
                valid_request_information.session_id,
            );

            response
                .headers
                .try_insert(
                    http::header::SET_COOKIE,
                    http::HeaderValue::from_str(
                        &cookie
                            .to_cookie_header(&self.cookie_settings)
                            .map_err(|e| {
                                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(
                                    e.to_string(),
                                )
                            })?,
                    )
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        }

        response
            .headers
            .try_insert(
                self.proxy_header_name.clone(),
                self.proxy_header_value.clone(),
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        ctx.upstream_response_body_seen = false;
        ctx.fallback_error_page_body = None;
        if (response.status.is_client_error() || response.status.is_server_error())
            && Self::has_explicitly_empty_body(&response.headers)
            && let Some(page_bytes) = Self::render_error_page_html(response.status.as_u16())
        {
            response.headers.remove(http::header::CONTENT_LENGTH);
            response.headers.remove(http::header::CONTENT_TYPE);
            response
                .headers
                .try_insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("text/html; charset=utf-8"),
                )
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
            ctx.fallback_error_page_body = Some(page_bytes);
        }

        Ok(())
    }
}
