#[allow(clippy::too_many_arguments)]
pub fn create_service(
    config: ::std::sync::Arc<ksbh_core::Config>,
    tls_settings: pingora::listeners::tls::TlsSettings,
    storage: ::std::sync::Arc<ksbh_core::Storage>,
    public_config: ksbh_types::PublicConfig,
    hosts: ksbh_core::routing::RouterReader,
    modules_configs_registry: ksbh_core::modules::registry::ModuleRegistryReader,
    metrics_sender: tokio::sync::mpsc::Sender<ksbh_core::metrics::RequestMetrics>,
    sessions: ::std::sync::Arc<
        ksbh_core::storage::redis_hashmap::RedisHashMap<
            ksbh_core::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    modules: ::std::sync::Arc<ksbh_core::modules::abi::module_host::ModuleHost>,
) -> pingora::services::listening::Service<
    pingora::proxy::HttpProxy<crate::proxy::PingoraWrapper<ksbh_core::proxy::ProxyService>>,
> {
    let pingora_server_conf = ::std::sync::Arc::new(config.to_server_conf().validate().unwrap());

    let proxy_wrapper = crate::proxy::PingoraWrapper::new(ksbh_core::proxy::ProxyService::new(
        config.clone(),
        storage,
        public_config,
        hosts,
        modules_configs_registry,
        metrics_sender,
        sessions,
        modules,
    ));

    let mut proxy = pingora::proxy::http_proxy_service_with_name(
        &pingora_server_conf,
        proxy_wrapper,
        "HttpProxy",
    );

    proxy.add_tcp(&config.listen_address.to_string());
    proxy.add_tls_with_settings(&config.listen_address_tls.to_string(), None, tls_settings);

    proxy
}
