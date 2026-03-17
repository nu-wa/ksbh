use std::sync::Arc;

pub fn start_pingora(
    config: ksbh_core::Config,
    storage: ::std::sync::Arc<ksbh_core::Storage>,
    _guard: tracing_appender::non_blocking::WorkerGuard,
) -> anyhow::Result<()> {
    let (tx, rx) = tokio::sync::mpsc::channel(1024);

    // Sessions for both proxy and modules (key = ModuleSessionKey)
    let sessions: ksbh_core::storage::redis_hashmap::RedisHashMap<
        ksbh_core::storage::module_session_key::ModuleSessionKey,
        Vec<u8>,
    > = ksbh_core::storage::redis_hashmap::RedisHashMap::new(
        Some(::std::time::Duration::from_hours(24)),
        Some(::std::time::Duration::from_hours(48)),
        Some(storage.clone()),
    );

    let sessions = ::std::sync::Arc::new(sessions);
    let (metrics_w, _metrics_r) = ksbh_core::metrics::Metrics::create(sessions.clone());
    let modules = ::std::sync::Arc::new(ksbh_core::modules::abi::module_host::ModuleHost::new(
        sessions.clone(),
    ));
    let (router_r, router_w) = ksbh_core::routing::Router::create();
    let (certs_r, certs_w) = ksbh_core::certs::CertsRegistry::create();
    let (modules_r, _modules_w) = ksbh_core::modules::registry::ModuleRegistry::create();

    // Determine which config provider to use based on KSBH__CONFIG_PATHS__CONFIG env var
    let config_provider: Box<dyn ksbh_core::config_provider::ConfigProvider> =
        match ksbh_core::utils::get_env_prefer_file("KSBH__CONFIG_PATHS__CONFIG") {
            Ok(config_path) => {
                tracing::info!(
                    "Using file-based config provider with path: {}",
                    config_path
                );
                Box::new(ksbh_config_providers_file::FileConfigProvider::new(
                    std::path::PathBuf::from(config_path),
                ))
            }
            Err(_) => {
                tracing::info!("Using kubernetes config provider");
                Box::new(ksbh_config_providers_kubernetes::KubeConfigProvider::new())
            }
        };

    let mut config_service = pingora::services::background::background_service(
        "config_service",
        ksbh_core::config_provider::ConfigService::new(config_provider, router_w, certs_w),
    );

    config_service.threads = Some(1);

    let mut pingora_server = pingora::server::Server::new_with_opt_and_conf(
        pingora::server::configuration::Opt {
            daemon: false,
            ..Default::default()
        },
        config
            .to_server_conf()
            .validate()
            .map_err(|e| ::std::io::Error::new(::std::io::ErrorKind::InvalidData, e))?,
    );

    let dynamic_cert = Box::new(crate::tls::DynamicTLS::new(certs_r));
    let mut tls_settings = pingora::listeners::tls::TlsSettings::with_callbacks(dynamic_cert)
        .expect("Could not create TLS settings");
    tls_settings
        .set_ciphersuites(
            "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256",
        )
        .expect("Could not set ciphersuites");
    tls_settings.enable_h2();

    let config = Arc::new(config);

    let mut prom_service = pingora::services::listening::Service::prometheus_http_service();
    prom_service.threads = Some(1);

    prom_service.add_tcp(&config.listen_addresses.prometheus.to_string());

    let mut static_internal = crate::apps::static_content::static_http_service(config.clone());
    static_internal.add_tcp(&config.listen_addresses.internal.to_string());
    static_internal.threads = Some(2);

    let mut services: Vec<Box<dyn pingora::services::Service>> = vec![
        Box::new(static_internal),
        Box::new(config_service),
        Box::new({
            let mut bg_service = pingora::services::background::background_service(
                "background service (web server)",
                crate::services::BackgroundService::new(
                    modules.clone(),
                    config.clone(),
                    sessions.clone(),
                ),
            );
            bg_service.threads = Some(1);
            bg_service
        }),
        Box::new({
            let mut metrics_service = pingora::services::background::background_service(
                "metrics service",
                crate::services::metrics::MetricsService::new(rx, metrics_w),
            );
            metrics_service.threads = Some(1);
            metrics_service
        }),
        Box::new(prom_service),
        Box::new({
            let mut proxy_service = crate::proxy::proxy_service::create_service(
                config.clone(),
                tls_settings,
                storage,
                router_r,
                modules_r,
                tx,
                sessions,
                modules,
            );
            proxy_service.threads = Some(config.threads);
            proxy_service
        }),
    ];

    #[cfg(feature = "profiling")]
    {
        let mut profiling_service = crate::profiling::profiling_service();
        profiling_service.add_tcp(&config.listen_addresses.profiling.to_string());
        profiling_service.threads = Some(1);
        services.push(Box::new(profiling_service));
    }

    pingora_server.add_services(services);
    pingora_server.bootstrap();
    pingora_server.run(pingora::server::RunArgs::default());
    Ok(())
}
