use ksbh_core::routing::{
    Backend, HostRules, IngressData, IngressRule, IngressRecipe, ModuleData, PathType, TlsData,
};
use ksbh_types::KsbhStr;

#[derive(Debug, Clone)]
pub struct FileProvider {
    pub config_path: ::std::path::PathBuf,
}

impl FileProvider {
    pub fn new(p: &str) -> Self {
        Self {
            config_path: ::std::path::PathBuf::from(p),
        }
    }

    /// Read the on-disk YAML and translate it into a list of [`IngressData`].
    fn load_ingress_data(
        config_path: &::std::path::Path,
    ) -> Result<Vec<IngressData>, ksbh_core::config_provider::ConfigProviderError> {
        let config = crate::config::FileConfig::load(config_path)
            .map_err(|e| Box::new(e) as ksbh_core::config_provider::ConfigProviderError)?;

        let modules: Vec<ModuleData> = config
            .modules
            .as_ref()
            .map(|m| m.iter().map(build_module_data).collect())
            .unwrap_or_default();

        let ingresses = config
            .ingresses
            .iter()
            .map(|ingress| build_ingress_data(ingress, modules.clone()))
            .collect();

        Ok(ingresses)
    }
}

fn build_module_data(
    module: &crate::config::modules::FileConfigModules,
) -> ModuleData {
    let mut mod_config: hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr> =
        hashbrown::HashMap::new();

    for (k, v) in &module.config {
        let value = KsbhStr::new({
            if let Some(value) = v.strip_prefix('$')
                && let Ok(env_value) = ksbh_core::utils::get_env_prefer_file(value)
            {
                env_value
            } else {
                v.to_string()
            }
        });

        mod_config.insert(KsbhStr::new(k), value);
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

    ModuleData {
        name: module.name.clone(),
        spec: mod_spec,
        values: mod_config,
    }
}

fn resolve_env_path(value: &str) -> String {
    if let Some(env_key) = value.strip_prefix('$')
        && let Ok(env_value) = ksbh_core::utils::get_env_prefer_file(env_key)
    {
        return env_value;
    }
    value.to_string()
}

fn build_ingress_data(
    ingress: &crate::config::ingress::FileConfigIngress,
    modules: Vec<ModuleData>,
) -> IngressData {
    let mut tls_entries: Vec<TlsData> = Vec::new();

    if let Some(ref tls) = ingress.tls {
        let cert_file = resolve_env_path(&tls.cert_file);
        let key_file = resolve_env_path(&tls.key_file);

        match ::std::fs::read_to_string(&cert_file) {
            Ok(cert_content) => match ::std::fs::read_to_string(&key_file) {
                Ok(key_content) => {
                    tls_entries.push(TlsData {
                        name: ingress.name.clone(),
                        cert_pem: cert_content,
                        key_pem: key_content,
                    });
                }
                Err(e) => tracing::error!("Failed to read key content {e}"),
            },
            Err(e) => tracing::error!("Failed to read cert content {e}"),
        }
    }

    let mut rules: Vec<IngressRule> = Vec::new();

    for path in &ingress.paths {
        let backend_str = path.backend.to_lowercase();

        let backend = match backend_str.as_str() {
            "service" => {
                if let Some(ref svc) = path.service {
                    Backend::Upstream {
                        name: KsbhStr::new(&svc.name),
                        port: svc.port,
                    }
                } else {
                    tracing::error!("Missing service information");
                    continue;
                }
            }
            "static" => Backend::Static,
            _ => {
                tracing::warn!(
                    "invalid backend type (should be service, static) got: {}",
                    backend_str
                );
                continue;
            }
        };

        let path_type_str = path.r#type.to_lowercase();
        let path_type = match path_type_str.as_str() {
            "exact" => PathType::Exact,
            "prefix" => PathType::Prefix,
            "implementationspecific" => PathType::ImplementationSpecific,
            _ => {
                tracing::warn!(
                    "Invalid path type (should be exact, prefix, implementationSpecific) got: {}",
                    path_type_str
                );
                continue;
            }
        };

        rules.push(IngressRule {
            path: path.path.clone(),
            path_type,
            backend,
        });
    }

    let peer_options = ingress.peer_options.as_ref().map(|po| {
        ksbh_types::providers::proxy::peer_options::PeerOptions {
            altnerative_names: po
                .alternative_names
                .clone()
                .map(|an| {
                    an.into_iter()
                        .map(|s| ::std::sync::Arc::<str>::from(s))
                        .collect()
                })
                .unwrap_or_default(),
            sni: po.sni.clone().map(|s| ::std::sync::Arc::from(s.as_str())),
            verify_cert: po.verify_cert,
        }
    });

    IngressData {
        name: ingress.name.clone(),
        hosts: vec![HostRules {
            host: ingress.host.clone(),
            rules,
        }],
        tls: tls_entries,
        attached_modules: ingress.modules.clone(),
        excluded_modules: ingress.excluded_modules.clone(),
        peer_options,
        modules,
    }
}

#[::async_trait::async_trait]
impl ksbh_core::config_provider::ConfigProvider for FileProvider {
    async fn emit(
        &self,
        router: &mut ksbh_core::routing::RouterWriter,
        certs: &mut ksbh_core::certs::CertsWriter,
        shutdown: pingora_core::server::ShutdownWatch,
    ) -> Result<(), ksbh_core::config_provider::ConfigProviderError> {
        let target_filename = self
            .config_path
            .file_name()
            .map(|s| s.to_os_string())
            .unwrap_or_default();

        let parent = self
            .config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| ::std::path::PathBuf::from("."));

        let watch_path = parent.clone();

        let router = router.clone();
        let certs = certs.clone();

        let path = self.config_path.clone();
        let target_filename_entry = target_filename.clone();
        let router_entry = router.clone();
        let certs_entry = certs.clone();
        let entry_fn = move |entry: ksbh_core::walkdir::DirEntry| {
            let path = path.clone();
            let target_filename = target_filename_entry.clone();
            let mut router = router_entry.clone();
            let mut certs = certs_entry.clone();
            async move {
                let _ = target_filename;
                if entry.path() == path.as_path() {
                    let data = match FileProvider::load_ingress_data(&path) {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::error!("Failed to load config {}: {e}", path.display());
                            return;
                        }
                    };
                    for ingress in &data {
                        if let Err(e) = IngressRecipe::emit(ingress, &mut router, &mut certs).await
                        {
                            tracing::error!("Failed to emit ingress '{}': {e}", ingress.name);
                        }
                    }
                }
            }
        };

        let path = self.config_path.clone();
        let router_notify = router.clone();
        let certs_notify = certs.clone();
        let notify_fn = move |event: ksbh_core::notify::Event| {
            let path = path.clone();
            let target_filename = target_filename.clone();
            let mut router = router_notify.clone();
            let mut certs = certs_notify.clone();
            async move {
                let event_targets_target = !target_filename.is_empty()
                    && event.paths.iter().any(|p| {
                        p.file_name()
                            .map(|n| n == target_filename.as_os_str())
                            .unwrap_or(false)
                    });
                if !event_targets_target {
                    return;
                }
                let data = match FileProvider::load_ingress_data(&path) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::error!("Failed to load config {}: {e}", path.display());
                        return;
                    }
                };
                for ingress in &data {
                    if let Err(e) = IngressRecipe::emit(ingress, &mut router, &mut certs).await {
                        tracing::error!("Failed to emit ingress '{}': {e}", ingress.name);
                    }
                }
            }
        };

        ksbh_core::utils::watch_directory_files_async(
            watch_path,
            entry_fn,
            notify_fn,
            Some(shutdown),
        )
        .await
        .map_err(|e| Box::new(e) as ksbh_core::config_provider::ConfigProviderError)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
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

        use ksbh_core::config_provider::ConfigProvider;

        let mut router_writer = router_writer;
        let mut certs_writer = certs_writer;

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let emit_handle = tokio::spawn(async move {
            file_config_provider
                .emit(&mut router_writer, &mut certs_writer, shutdown_rx)
                .await
        });

        tokio::time::sleep(::std::time::Duration::from_millis(500)).await;

        let http_request =
            ksbh_types::prelude::HttpRequest::t_create("local.host", Some(b"/"), None);

        let global_modules = router_reader.get_global_modules_configs();
        assert!(!global_modules.is_empty());

        let route = router_reader.find_route(&http_request);
        assert!(route.is_some());

        let _ = shutdown_tx.send(true);
        let _ = emit_handle.await;
    }
}
