#[derive(Debug)]
pub struct FileProvider {
    pub config_path: ::std::path::PathBuf,
}

impl FileProvider {
    pub fn new(p: &str) -> Self {
        Self {
            config_path: ::std::path::PathBuf::from(p),
        }
    }
}

#[::async_trait::async_trait]
impl ksbh_core::config_provider::ConfigProvider for FileProvider {
    async fn start(
        &self,
        router: ksbh_core::routing::RouterWriter,
        certs: ksbh_core::certs::CertsWriter,
        _shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let config =
            crate::config::FileConfig::load(&self.config_path).expect("Invalid file configuration");

        if let Some(modules) = &config.modules {
            for module in modules.iter() {
                let mut mod_config: hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr> =
                    hashbrown::HashMap::new();

                for (k, v) in &module.config {
                    // Load special values from environment variables
                    let value = ksbh_types::KsbhStr::new({
                        if let Some(value) = v.strip_prefix('$')
                            && let Ok(env_value) = ksbh_core::utils::get_env_prefer_file(value)
                        {
                            env_value
                        } else {
                            v.to_string()
                        }
                    });

                    mod_config.insert(ksbh_types::KsbhStr::new(k), value);
                }

                let mod_config = ::std::sync::Arc::new(mod_config);

                let mod_spec = ksbh_core::modules::ModuleConfigurationSpec {
                    config: None,
                    global: module.global,
                    name: module.name.clone(),
                    requires_body: module.requires_body,
                    secret_ref: None,
                    r#type: module.r#type.to_owned(),
                    weight: module.weight,
                };
                router.upsert_module(&module.name, module.global, mod_config, mod_spec);
            }
        }

        for ingress in config.ingresses {
            let mut hosts = Vec::new();
            let mut host_paths = ksbh_core::routing::HostPaths::default();

            for path in ingress.paths {
                let backend = path.backend.to_lowercase();

                let backend = match backend.as_str() {
                    "service" => {
                        if let Some(ref svc) = path.service {
                            ksbh_core::routing::ServiceBackendType::ServiceBackend(
                                ksbh_core::routing::ServiceBackend {
                                    name: ksbh_types::KsbhStr::new(&svc.name),
                                    port: svc.port,
                                },
                            )
                        } else {
                            tracing::error!("Missing service information");
                            continue;
                        }
                    }
                    "static" => ksbh_core::routing::ServiceBackendType::Static,
                    _ => {
                        tracing::warn!(
                            "invalid backend type (should be service, static, self) got: {}",
                            backend
                        );
                        continue;
                    }
                };

                let path_key = ksbh_types::KsbhStr::new(&path.path);

                let path_type = path.r#type.to_lowercase();

                match path_type.as_str() {
                    "exact" => {
                        host_paths.exact.insert(path_key, backend);
                    }
                    "prefix" => {
                        host_paths.prefix.push((path_key, backend));
                    }
                    _ => {
                        tracing::warn!(
                            "Invalid path type (should be exact, prefix, implementationSpecific) got: {}",
                            path_type
                        );
                    }
                }
            }

            if let Some(ref tls) = ingress.tls {
                let cert_file = if let Some(value) = tls.cert_file.strip_prefix('$')
                    && let Ok(env_value) = ksbh_core::utils::get_env_prefer_file(value)
                {
                    env_value
                } else {
                    tls.cert_file.to_string()
                };
                let key_file = if let Some(value) = tls.key_file.strip_prefix('$')
                    && let Ok(env_value) = ksbh_core::utils::get_env_prefer_file(value)
                {
                    env_value
                } else {
                    tls.cert_file.to_string()
                };
                let cert_content = match ::std::fs::read_to_string(cert_file) {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::error!("Failed to read cert content {e}");
                        continue;
                    }
                };
                let key_content = match ::std::fs::read_to_string(key_file) {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::error!("Failed to read cert content {e}");
                        continue;
                    }
                };

                if let Err(e) =
                    load_tls_cert(&certs, &ingress.name, cert_content, key_content).await
                {
                    tracing::error!("Failed to parse cert for ingress: {}, {}", ingress.name, e);
                }
            }

            hosts.push((::std::sync::Arc::from(ingress.host.as_str()), host_paths));

            router.insert_ingress(
                &ingress.name,
                hosts,
                ksbh_core::routing::IngressModuleConfig {
                    modules: ingress
                        .modules
                        .iter()
                        .map(|s| ::std::sync::Arc::from(s.as_str()))
                        .collect(),
                    excluded_modules: ingress
                        .excluded_modules
                        .iter()
                        .map(|s| ::std::sync::Arc::from(s.as_str()))
                        .collect(),
                },
                ingress.https.unwrap_or(false),
            );
        }
    }
}

async fn load_tls_cert(
    certs_writer: &ksbh_core::certs::CertsWriter,
    ingress_name: &str,
    cert: String,
    key: String,
) -> Result<(), Box<dyn ::std::error::Error>> {
    let private_key = pingora_core::tls::pkey::PKey::private_key_from_pem(key.as_bytes())?;
    let cert_chain = pingora_core::tls::x509::X509::stack_from_pem(cert.as_bytes())
        .map_err(|_e| ::std::io::Error::other("Failed to parse cert chain".to_string()))?;

    let (domains, wildcards) = ksbh_core::certs::extract_domains_from_cert(&cert_chain);
    if domains.is_empty() && wildcards.is_empty() {
        return Err("No SAN DNS names".into());
    }

    certs_writer
        .add_cert(ingress_name, private_key, cert_chain, domains, wildcards)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic() {
        let ctx = crate::test_utils::Context::new(
            "
modules:
  - name: 'some_module'
    global: false
    weight: 100
    type: robotsdottxt
    requires_body: false
  - name: 'some_other_module'
    weight: 200
    type: robotsdottxt
  - name: 'global_module'
    type: robotsdottxt
    weight: 300
    global: true
ingresses:
  - name: 'some_ingress'
    host: 'local.host'
    paths:
      - path: '/'
        type: 'prefix'
        backend: 'static'
        ",
        );

        let (_certs_reader, certs_writer) = ksbh_core::certs::CertsRegistry::create();
        let (router_reader, router_writer) = ksbh_core::routing::Router::create();

        let file_config_provider =
            crate::FileProvider::new(ctx.tmp_file.path().as_os_str().to_str().unwrap());
        let (_shutdown_signal_sender, shutdown_signal) = tokio::sync::watch::channel(false);

        use ksbh_core::config_provider::ConfigProvider;

        file_config_provider
            .start(router_writer, certs_writer, shutdown_signal)
            .await;

        let http_request =
            ksbh_types::prelude::HttpRequest::t_create("local.host", Some(b"/"), None);

        let global_modules = router_reader.get_global_modules_configs();

        assert!(!global_modules.is_empty());

        let route = router_reader.find_route(&http_request);

        assert!(route.is_some());
    }
}
