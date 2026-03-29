/// The main proxy service implementation combining routing, modules, storage, and metrics.
///
/// This struct implements the `ProxyProvider` trait from ksbh-types and orchestrates
/// the request lifecycle: request filtering, upstream peer resolution, header
/// manipulation, response filtering, and metrics collection.
#[allow(dead_code)]
pub struct ProxyService {
    #[allow(dead_code)]
    pub(super) storage: ::std::sync::Arc<crate::Storage>,
    #[allow(dead_code)]
    pub(super) sessions: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    #[allow(dead_code)]
    pub(super) config: ::std::sync::Arc<crate::Config>,
    #[allow(dead_code)]
    pub(super) hosts: crate::routing::RouterReader,
    #[allow(dead_code)]
    pub(super) metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
    #[allow(dead_code)]
    pub(super) modules: ::std::sync::Arc<crate::modules::abi::module_host::ModuleHost>,
    pub(super) cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    pub(super) proxy_header_name: http::header::HeaderName,
    pub(super) proxy_header_value: http::HeaderValue,
}

impl ProxyService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: ::std::sync::Arc<crate::Config>,
        storage: ::std::sync::Arc<crate::Storage>,
        hosts: crate::routing::RouterReader,
        metrics_sender: tokio::sync::mpsc::Sender<crate::metrics::RequestMetrics>,
        sessions: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
        modules: ::std::sync::Arc<crate::modules::abi::module_host::ModuleHost>,
        cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    ) -> Self {
        let proxy_header_name =
            http::header::HeaderName::from_bytes(config.constants.proxy_header_name.as_bytes())
                .expect("validated proxy header name must parse");
        let proxy_header_value = http::HeaderValue::from_str(&config.constants.proxy_header_value)
            .expect("validated proxy header value must parse");

        Self {
            modules,
            storage,
            sessions,
            config: config.clone(),
            hosts,
            metrics_sender,
            cookie_settings,
            proxy_header_name,
            proxy_header_value,
        }
    }

    fn has_explicitly_empty_body(headers: &http::HeaderMap) -> bool {
        headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim() == "0")
            .unwrap_or(false)
    }

    fn render_error_page_html(status_code: u16) -> Option<bytes::Bytes> {
        ksbh_ui::error_pages::render_error_page_html(&status_code.to_string())
            .map(bytes::Bytes::from)
    }

    fn append_header_value_if_trusted(
        existing_value: Option<&http::header::HeaderValue>,
        appended_value: &str,
        trust_forwarded_headers: bool,
    ) -> String {
        if !trust_forwarded_headers {
            return appended_value.to_string();
        }

        let Some(existing_value) = existing_value else {
            return appended_value.to_string();
        };

        let Ok(existing_value) = existing_value.to_str() else {
            return appended_value.to_string();
        };
        let existing_value = existing_value.trim();
        if existing_value.is_empty() {
            return appended_value.to_string();
        }

        format!("{existing_value}, {appended_value}")
    }

    fn format_forwarded_for_value(ip: &::std::net::IpAddr) -> String {
        match ip {
            ::std::net::IpAddr::V4(v4) => v4.to_string(),
            ::std::net::IpAddr::V6(v6) => format!("\"[{v6}]\""),
        }
    }

    fn escape_forwarded_value(raw: &str) -> String {
        let mut escaped = String::with_capacity(raw.len());
        for char in raw.chars() {
            match char {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                _ => escaped.push(char),
            }
        }

        escaped
    }

    fn forwarded_header_entry(
        client_ip: Option<::std::net::IpAddr>,
        proto: &str,
        host: &str,
    ) -> String {
        let mut parts = Vec::with_capacity(3);

        match client_ip {
            Some(ip) => parts.push(format!("for={}", Self::format_forwarded_for_value(&ip))),
            None => parts.push("for=unknown".to_string()),
        };

        parts.push(format!("proto=\"{}\"", Self::escape_forwarded_value(proto)));
        parts.push(format!("host=\"{}\"", Self::escape_forwarded_value(host)));

        parts.join(";")
    }

    fn normalize_cookie_header_for_upstream(
        upstream_request: &mut pingora_http::RequestHeader,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        let merged_cookie = upstream_request
            .headers
            .get_all(http::header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<&str>>()
            .join("; ");

        if merged_cookie.is_empty() {
            return Ok(());
        }

        upstream_request.remove_header(&http::header::COOKIE);
        upstream_request
            .insert_header(
                http::header::COOKIE,
                http::HeaderValue::from_str(merged_cookie.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ksbh_types::prelude::ProxyProvider for ProxyService {
    type ProxyContext = crate::proxy::ProxyContext;

    fn new_context(&self) -> Self::ProxyContext {
        crate::proxy::ProxyContext::new(self.config.clone())
    }

    async fn request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> ksbh_types::prelude::ProxyProviderResult {
        Ok(self._request_filter(session, ctx).await?)
    }

    async fn upstream_peer(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        ctx: &mut Self::ProxyContext,
    ) -> Result<ksbh_types::providers::proxy::UpstreamPeer, ksbh_types::prelude::ProxyProviderError>
    {
        use std::str::FromStr;
        let internal_upstream_address = self
            .config
            .listen_addresses
            .internal_connect_addr()
            .to_string();

        // Return to internal error page
        if let Some(ksbh_types::prelude::ProxyDecision::StopProcessing(
            decision_code,
            _decision_msg,
        )) = &ctx.proxy_decision
        {
            let uri = ::std::format!("http://internal.ksbh.rs/{}", decision_code.as_str());

            session.set_request_uri(
                http::Uri::from_str(uri.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            );

            return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                address: internal_upstream_address.clone(),
            });
        }

        if let Some(valid_request_information) = &ctx.valid_request_information {
            return match &valid_request_information.req_match.backend {
                crate::routing::ServiceBackendType::ServiceBackend(svc) => {
                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: format!("{}:{}", svc.name, svc.port),
                    })
                }
                crate::routing::ServiceBackendType::Static => {
                    let http_request = match &ctx.http_request {
                        Some(req) => req,
                        None => {
                            session.set_request_uri(
                                http::Uri::from_str("http://internal.ksbh.rs/500")
                                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                            );

                            return Ok(ksbh_types::providers::proxy::UpstreamPeer {
                                address: internal_upstream_address.clone(),
                            });
                        }
                    };
                    let request_path = urlencoding::encode(&http_request.query.path);
                    let new_path = format!("/static?path={request_path}");

                    session.set_request_uri(
                        http::Uri::from_str(&new_path)
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: internal_upstream_address.clone(),
                    })
                }
                _ => {
                    session.set_request_uri(
                        http::Uri::from_str("http://internal.ksbh.rs/500")
                            .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
                    );

                    Ok(ksbh_types::providers::proxy::UpstreamPeer {
                        address: internal_upstream_address.clone(),
                    })
                }
            };
        }

        // No request match, no module replied, or invalid request
        session.set_request_uri(
            http::Uri::from_str("http://internal.ksbh.rs/404")
                .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
        );

        return Ok(ksbh_types::providers::proxy::UpstreamPeer {
            address: internal_upstream_address,
        });
    }

    async fn response_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        response: &mut http::response::Parts,
        ctx: &mut Self::ProxyContext,
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

        if ctx.downstream_ws_kind != crate::proxy::DownstreamWebsocketKind::None {
            response
                .headers
                .try_insert(
                    crate::constants::HEADER_X_KSBH_WS_DOWNSTREAM_TRANSPORT,
                    http::HeaderValue::from_str(ctx.downstream_transport.as_str())
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

    fn response_body_filter(
        &self,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
        if body.as_ref().is_some_and(|chunk| !chunk.is_empty()) {
            ctx.upstream_response_body_seen = true;
        }

        if end_of_stream
            && !ctx.upstream_response_body_seen
            && body.as_ref().is_none_or(bytes::Bytes::is_empty)
            && let Some(fallback_body) = ctx.fallback_error_page_body.take()
        {
            *body = Some(fallback_body);
            ctx.upstream_response_body_seen = true;
        }

        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        upstream_request: &mut pingora_http::RequestHeader,
        ctx: &mut Self::ProxyContext,
    ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
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
        let upstream_host_with_port = ctx
            .valid_request_information
            .as_ref()
            .and_then(|valid_request_information| {
                match &valid_request_information.req_match.backend {
                    crate::routing::ServiceBackendType::ServiceBackend(service_backend) => Some(
                        if service_backend.port == 80 || service_backend.port == 443 {
                            service_backend.name.to_string()
                        } else {
                            format!("{}:{}", service_backend.name, service_backend.port)
                        },
                    ),
                    _ => None,
                }
            })
            .unwrap_or_else(|| host_with_port.clone());

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
            let forwarded_for = Self::append_header_value_if_trusted(
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
                http::HeaderValue::from_str(upstream_host_with_port.as_str())
                    .map_err(ksbh_types::prelude::ProxyProviderError::from)?,
            )
            .map_err(ksbh_types::prelude::ProxyProviderError::from)?;
        let forwarded_entry = Self::forwarded_header_entry(
            direct_client_ip.or(effective_client_ip),
            proto,
            &host_with_port,
        );
        let forwarded_value = Self::append_header_value_if_trusted(
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

    async fn logging(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        _error: Option<&ksbh_types::prelude::ProxyProviderError>,
        ctx: &mut Self::ProxyContext,
    ) {
        let duration = ctx.req_start.elapsed();

        if let Some(valid_req_information) = ctx.valid_request_information.take()
            && let Some(response_status) = session.response_status()
            && let Err(e) = self
                .metrics_sender
                .send(crate::metrics::RequestMetrics::new(
                    valid_req_information,
                    ::std::mem::take(&mut ctx.modules_metrics),
                    response_status,
                    duration.as_secs_f64(),
                ))
                .await
        {
            tracing::error!("There was an error sending request_metric {}", e);
        }
    }

    async fn fail_to_proxy(
        &self,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        error_code: u16,
        _ctx: &mut Self::ProxyContext,
    ) -> Result<bool, ksbh_types::prelude::ProxyProviderError> {
        if !(400..=599).contains(&error_code) {
            return Ok(false);
        }

        let Some(body) = Self::render_error_page_html(error_code) else {
            return Ok(false);
        };
        let status = http::StatusCode::from_u16(error_code)
            .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
        let response = http::Response::builder()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(body)
            .map_err(|e| {
                ksbh_types::prelude::ProxyProviderError::InternalErrorDetailed(e.to_string())
            })?;

        session.write_response(response).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalize_cookie_header_for_upstream_merges_multiple_headers() {
        let mut request = pingora_http::RequestHeader::build_no_case(
            http::Method::GET,
            b"/",
            Some(4),
        )
        .expect("request build should succeed");
        request
            .append_header(http::header::COOKIE, "ksbh=abc")
            .expect("append cookie should succeed");
        request
            .append_header(http::header::COOKIE, "authentik_session=def")
            .expect("append cookie should succeed");

        super::ProxyService::normalize_cookie_header_for_upstream(&mut request)
            .expect("normalization should succeed");

        let cookies: Vec<&http::HeaderValue> =
            request.headers.get_all(http::header::COOKIE).iter().collect();
        assert_eq!(cookies.len(), 1);
        assert_eq!(
            cookies[0].to_str().expect("cookie must be utf-8"),
            "ksbh=abc; authentik_session=def"
        );
    }

    #[test]
    fn append_header_value_ignores_untrusted_existing_value() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            crate::constants::HEADER_X_FORWARDED_FOR,
            http::HeaderValue::from_static("198.51.100.9"),
        );

        let appended = super::ProxyService::append_header_value_if_trusted(
            headers.get(crate::constants::HEADER_X_FORWARDED_FOR),
            "203.0.113.8",
            false,
        );

        assert_eq!(appended, "203.0.113.8");
    }

    #[test]
    fn append_header_value_appends_for_trusted_proxy() {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            crate::constants::HEADER_X_FORWARDED_FOR,
            http::HeaderValue::from_static("198.51.100.9"),
        );

        let appended = super::ProxyService::append_header_value_if_trusted(
            headers.get(crate::constants::HEADER_X_FORWARDED_FOR),
            "203.0.113.8",
            true,
        );

        assert_eq!(appended, "198.51.100.9, 203.0.113.8");
    }

    #[test]
    fn forwarded_entry_formats_ipv6_and_quotes_host_and_proto() {
        let entry = super::ProxyService::forwarded_header_entry(
            Some("2001:db8::1".parse().expect("parse IPv6 address")),
            "https",
            "example.test:443",
        );

        assert_eq!(
            entry,
            "for=\"[2001:db8::1]\";proto=\"https\";host=\"example.test:443\""
        );
    }
}
