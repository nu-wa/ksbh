pub(crate) mod error_pages;
pub(crate) mod healthz;
pub(crate) mod static_app;

pub struct StaticHttpApp {
    healthz: ::std::sync::Arc<healthz::HealthzApp>,
    error_pages: ::std::sync::Arc<error_pages::ErrorPagesApp>,
    static_app: ::std::sync::Arc<static_app::StaticApp>,
}

impl StaticHttpApp {
    pub fn new(
        config: ::std::sync::Arc<ksbh_core::Config>,
    ) -> Result<Self, error_pages::ErrorPagesAppError> {
        Ok(Self {
            healthz: ::std::sync::Arc::new(healthz::HealthzApp),
            error_pages: ::std::sync::Arc::new(error_pages::ErrorPagesApp::new()?),
            static_app: ::std::sync::Arc::new(static_app::StaticApp::new(config)),
        })
    }
}

#[::async_trait::async_trait]
impl pingora::apps::HttpServerApp for StaticHttpApp {
    async fn process_new_http(
        self: &::std::sync::Arc<Self>,
        mut session: pingora::protocols::http::ServerSession,
        shutdown: &pingora::server::ShutdownWatch,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        tracing::span!(tracing::Level::DEBUG, "StaticHttpApp_process_new_http");
        match session.read_request().await {
            Ok(success) => {
                tracing::debug!("StaticHttpApp: read_request: {success}");
            }
            Err(e) => {
                tracing::error!("{:?}", e);

                return None;
            }
        };

        let req_id = uuid::Uuid::new_v4();
        let req_headers = session.req_header();
        let trust_forwarded_headers = self.static_app.config.trusts_forwarded_headers_from(
            session
                .client_addr()
                .and_then(|addr| addr.as_inet().map(|sock_addr| sock_addr.ip())),
        );
        let downstream_tls = session
            .server_addr()
            .and_then(|addr| addr.as_inet().map(::std::net::SocketAddr::port))
            .map(|port| port == self.static_app.config.listen_addresses.https.port())
            .unwrap_or(false);

        let http_request_info = match ksbh_types::requests::http_request::HttpRequest::new(
            req_headers,
            req_id,
            &self.static_app.config.ports.external,
            downstream_tls,
            trust_forwarded_headers,
        ) {
            Ok(info) => info,
            Err(e) => {
                tracing::error!("{:?}", e);

                return None;
            }
        };

        let method_str = http_request_info.method.as_str();
        let head_only = method_str == "HEAD";
        let host = http_request_info.host.to_string();
        let path = http_request_info.query.path.to_string();
        let request_path_param = http_request_info
            .query
            .get_param("path")
            .map(|s| s.to_string());
        let file_param = http_request_info
            .query
            .get_param("file")
            .map(|s| s.to_string());

        tracing::debug!(
            "http_request_info: scheme={} method={} path={}",
            http_request_info.scheme.as_str(),
            method_str,
            path
        );

        if method_str == "GET" || head_only {
            match path.as_str() {
                "/healthz" => return self.healthz.handle_healthz(session, head_only).await,
                "/static" => {
                    return self
                        .static_app
                        .render_static_file(
                            session,
                            shutdown,
                            host.as_str(),
                            request_path_param.as_deref(),
                            file_param.as_deref(),
                            head_only,
                        )
                        .await;
                }
                "/400" => {
                    return self.error_pages.send(session, 400, head_only).await;
                }
                "/401" => {
                    return self.error_pages.send(session, 401, head_only).await;
                }
                "/403" => {
                    return self.error_pages.send(session, 403, head_only).await;
                }
                "/500" => {
                    return self.error_pages.send(session, 500, head_only).await;
                }
                "/502" => {
                    return self.error_pages.send(session, 502, head_only).await;
                }
                _ => {
                    return self.error_pages.send(session, 404, head_only).await;
                }
            };
        }

        match path.as_str() {
            "/static" | "/healthz" => self.error_pages.send_405(session, head_only).await,
            _ => self.error_pages.send(session, 404, false).await,
        }
    }
}

pub fn static_http_service(
    config: ::std::sync::Arc<ksbh_core::Config>,
) -> pingora::services::listening::Service<StaticHttpApp> {
    pingora::services::listening::Service::new(
        "static_service".to_string(),
        StaticHttpApp::new(config).expect("Could not create StaticHttpApp"),
    )
}
