use ::std::sync::Arc;

mod early_request_filter;
mod request_filter;

pub struct ProxyService {
    storage: Arc<ksbh_core::Storage>,
    sessions: Arc<
        ksbh_core::storage::redis_hashmap::RedisHashMap<
            ksbh_core::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    // TODO: Maybe bring this back one day.
    // extism_plugin_cache: ksbh_core::plugin::ExtismPluginCache,
    modules: ksbh_core::modules::registry::ModuleRegistryReader,
    public_config: ksbh_types::prelude::PublicConfig,
    config: Arc<ksbh_core::Config>,
    hosts: ksbh_core::routing::RouterReader,
    metrics_sender: tokio::sync::mpsc::Sender<ksbh_core::metrics::RequestMetrics>,
}

impl ProxyService {
    #[allow(clippy::too_many_arguments)]
    pub fn create_service(
        config: Arc<ksbh_core::Config>,
        tls_settings: pingora::listeners::tls::TlsSettings,
        storage: Arc<ksbh_core::Storage>,
        public_config: ksbh_types::prelude::PublicConfig,
        hosts: ksbh_core::routing::RouterReader,
        modules: ksbh_core::modules::registry::ModuleRegistryReader,
        metrics_sender: tokio::sync::mpsc::Sender<ksbh_core::metrics::RequestMetrics>,
        sessions: Arc<
            ksbh_core::storage::redis_hashmap::RedisHashMap<
                ksbh_core::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    // TODO: Maybe bring this back one day.
    // extism_plugin_cache: ksbh_core::plugin::ExtismPluginCache,
    modules: ksbh_core::modules::registry::ModuleRegistryReader,
    public_config: ksbh_types::prelude::PublicConfig,
    config: Arc<ksbh_core::Config>,
    hosts: ksbh_core::routing::RouterReader,
    metrics_sender: tokio::sync::mpsc::Sender<ksbh_core::metrics::RequestMetrics>,
}

impl ProxyService {
    #[allow(clippy::too_many_arguments)]
    pub fn create_service(
        config: Arc<ksbh_core::Config>,
        tls_settings: pingora::listeners::tls::TlsSettings,
        storage: Arc<ksbh_core::Storage>,
        public_config: ksbh_types::prelude::PublicConfig,
        hosts: ksbh_core::routing::RouterReader,
        modules: ksbh_core::modules::registry::ModuleRegistryReader,
        metrics_sender: tokio::sync::mpsc::Sender<ksbh_core::metrics::RequestMetrics>,
        sessions: Arc<
            ksbh_core::storage::redis_hashmap::RedisHashMap<
                uuid::Uuid,
                Vec<u8>,
            >,
        >,
    ) -> pingora::services::listening::Service<pingora::proxy::HttpProxy<Self>> {
        let pingora_server_conf = Arc::new(config.to_server_conf().validate().unwrap());

        let mut proxy = pingora::proxy::http_proxy_service_with_name(
            &pingora_server_conf,
            Self {
                modules,
                storage,
                sessions,
                public_config,
                config: config.clone(),
                hosts,
                metrics_sender,
            },
            "HttpProxy",
        );

        proxy.add_tcp(&config.listen_address.to_string());
        proxy.add_tls_with_settings(&config.listen_address_tls.to_string(), None, tls_settings);

        proxy
    }
}

#[async_trait::async_trait]
impl pingora::proxy::ProxyHttp for ProxyService {
    type CTX = ksbh_core::proxy::ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        ksbh_core::proxy::ProxyContext::new(self.config.clone())
    }

    async fn early_request_filter(
        &self,
        session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<()> {
        self._early_request_filter(session, ctx).await
    }

    async fn request_filter(
        &self,
        session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<bool> {
        self._request_filter(session, ctx).await
    }

    async fn upstream_peer(
        &self,
        session: &mut pingora::proxy::Session,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<Box<pingora::upstreams::peer::HttpPeer>> {
        let valid_req_info = ctx.valid_request_information.clone().unwrap();

        let backend = ctx.backend.to_owned();

        match backend {
            ksbh_core::routing::ServiceBackendType::ServiceBackend(svc) => {
                let svc = svc.clone();
                let peer = pingora::prelude::HttpPeer::new(
                    (svc.name.as_str(), svc.port),
                    false,
                    svc.name.to_string(),
                );

                Ok(Box::new(peer))
            }
            ksbh_core::routing::ServiceBackendType::ToSelf(prefix) => {
                if let Some(prefix) = prefix {
                    let http_request_info = &valid_req_info.http_request_info;
                    let req_header = session.req_header_mut();
                    let mut path_bytes: Vec<u8> = prefix.as_bytes().to_vec();
                    path_bytes.extend_from_slice(http_request_info.host.0.as_bytes());
                    path_bytes.extend_from_slice(req_header.raw_path());
                    req_header.set_raw_path(&path_bytes).unwrap();
                }

                Ok(Box::new(pingora::prelude::HttpPeer::new(
                    self.config.listen_address_api,
                    false,
                    String::new(),
                )))
            }
            ksbh_core::routing::ServiceBackendType::Static => {
                let http_request_info = &valid_req_info.http_request_info;
                let req_header = session.req_header_mut();
                let file_path = format!(
                    "{}/{}",
                    http_request_info.host.as_str().trim_end_matches("/"),
                    http_request_info.query
                );
                let file_path = urlencoding::encode(&file_path);
                let new_path = format!("/static?file={}", file_path);
                req_header.set_raw_path(new_path.as_bytes()).unwrap();

                Ok(Box::new(pingora::prelude::HttpPeer::new(
                    self.config.listen_address_internal,
                    false,
                    String::new(),
                )))
            }
            ksbh_core::routing::ServiceBackendType::None => Err(pingora::Error::create(
                pingora::ErrorType::HTTPStatus(404),
                pingora::ErrorSource::Downstream,
                None,
                None,
            )),
        }
    }

    async fn response_filter(
        &self,
        session: &mut pingora::proxy::Session,
        response: &mut pingora::http::ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::prelude::Result<()> {
        if ctx.already_replied {
            return Ok(());
        }

        let valid_req_info = ctx.valid_request_information.clone().unwrap();

        if session
            .req_header()
            .headers
            .get(http::header::UPGRADE)
            .is_none()
            && !ctx.had_cookie
        {
            match valid_req_info.cookie.to_cookie_header() {
                Ok(header_string) => {
                    response.insert_header(http::header::SET_COOKIE, header_string)?;
                }
                Err(e) => {
                    tracing::error!("Failed to set cookie: {e}");
                }
            }
        }

        response.insert_header(
            ksbh_core::constants::PROXY_HEADER_NAME,
            ksbh_core::constants::PROXY_HEADER_VALUE,
        )?;

        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        session: &mut pingora::proxy::Session,
        upstream_request: &mut pingora::prelude::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        if ctx.already_replied {
            return Ok(());
        }

        let valid_req_info = ctx.valid_request_information.clone().unwrap();
        let http_req = &valid_req_info.http_request_info;

        let proto = if http_req.uri.as_str().starts_with("wss")
            || http_req.uri.as_str().starts_with("https")
        {
            "https"
        } else {
            "http"
        };
        upstream_request.insert_header("X-Forwarded-Proto", proto)?;
        let host_with_port = if http_req.port != 80 && http_req.port != 443 {
            format!("{}:{}", http_req.host, http_req.port)
        } else {
            http_req.host.to_string()
        };

        if let Some(client_addr) = ksbh_core::utils::get_client_ip_from_session(session) {
            upstream_request.insert_header("X-Forwarded-For", client_addr.to_string())?;
        }

        upstream_request.insert_header("X-Forwarded-Ssl", "on")?;
        upstream_request.insert_header("X-Forwarded-Host", host_with_port.as_str())?;
        upstream_request.insert_header("Host", host_with_port.as_str())?;
        if let Some(origin) = session.req_header().headers.get("Origin") {
            upstream_request.insert_header("Origin", origin)?;
        }

        Ok(())
    }

    async fn logging(
        &self,
        session: &mut pingora::proxy::Session,
        error: Option<&pingora::Error>,
        ctx: &mut Self::CTX,
    ) {
        let duration = ctx.req_start.elapsed();

        if let Some(client_information) = match ctx.valid_request_information.clone() {
            Some(valid_req_info) => Some((
                valid_req_info.http_request_info,
                valid_req_info.client_information,
            )),
            None => match ctx.partial_request_information.clone() {
                Some(partial_req_info) => Some((
                    partial_req_info.http_request_info,
                    partial_req_info.client_information,
                )),
                None => match ctx.early_request_information.clone() {
                    Some(early_req_info) => Some((
                        early_req_info.http_request_info,
                        early_req_info.client_information,
                    )),
                    None => None,
                },
            },
        } && let Some(response_header) = session.response_written()
            && let Err(e) = self
                .metrics_sender
                .send(ksbh_core::metrics::RequestMetrics::new(
                    client_information.1,
                    client_information.0,
                    ctx.backend.clone(),
                    true,
                    response_header.status,
                    error.map(|e| e.etype.to_owned()),
                    ctx.modules_metrics.clone(),
                    ctx.plugins_metrics.clone(),
                    duration.as_secs_f64(),
                ))
                .await
        {
            tracing::error!("There was an error sending request_metric {}", e);
        }
    }
}
