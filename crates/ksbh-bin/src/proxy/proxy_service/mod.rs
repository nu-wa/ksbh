#[allow(clippy::too_many_arguments)]
pub fn create_service(
    config: ::std::sync::Arc<ksbh_core::Config>,
    tls_settings: pingora::listeners::tls::TlsSettings,
    storage: ::std::sync::Arc<ksbh_core::Storage>,
    hosts: ksbh_core::routing::RouterReader,
    metrics_sender: tokio::sync::mpsc::Sender<ksbh_core::metrics::RequestMetrics>,
    sessions: ::std::sync::Arc<
        ksbh_core::storage::redis_hashmap::RedisHashMap<
            ksbh_core::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    modules: ::std::sync::Arc<ksbh_core::modules::abi::module_host::ModuleHost>,
    cookie_settings: ::std::sync::Arc<ksbh_core::cookies::CookieSettings>,
) -> pingora::services::listening::Service<
    pingora::proxy::HttpProxy<crate::proxy::PingoraWrapper<ksbh_core::proxy::ProxyService>>,
> {
    let pingora_server_conf = ::std::sync::Arc::new(
        config
            .to_server_conf()
            .validate()
            .expect("Invalid server configuration"),
    );

    let proxy_wrapper = crate::proxy::PingoraWrapper::new(ksbh_core::proxy::ProxyService::new(
        config.clone(),
        storage,
        hosts,
        metrics_sender,
        sessions,
        modules,
        cookie_settings,
    ));

    let mut proxy_inner = pingora::proxy::http_proxy(&pingora_server_conf, proxy_wrapper);
    let mut server_options = pingora::apps::HttpServerOptions::default();
    server_options.allow_connect_method_proxying = true;
    proxy_inner.server_options = Some(server_options);

    let mut h2_options = pingora::protocols::http::v2::server::H2Options::default();
    h2_options.enable_connect_protocol();
    proxy_inner.h2_options = Some(h2_options);

    let mut proxy =
        pingora::services::listening::Service::new("HttpProxy".to_string(), proxy_inner);

    let perf = &config.performance;
    let tcp_fastopen = perf
        .tcp_fastopen
        .unwrap_or(config.constants.tcp_fastopen_queue_size);
    let so_reuseport = perf.so_reuseport.unwrap_or(false);

    let mut sock_opts = pingora::listeners::TcpSocketOptions::default();
    sock_opts.tcp_fastopen = Some(tcp_fastopen);
    sock_opts.so_reuseport = Some(so_reuseport);

    proxy.add_tcp_with_settings(&config.listen_addresses.http.to_string(), sock_opts.clone());
    proxy.add_tls_with_settings(
        &config.listen_addresses.https.to_string(),
        None,
        tls_settings,
    );

    proxy
}
