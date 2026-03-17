pub use serde::{Deserialize, Serialize};

pub struct FileConfigProvider {
    config_path: ::std::path::PathBuf,
}

impl FileConfigProvider {
    pub fn new(config_path: ::std::path::PathBuf) -> Self {
        Self { config_path }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
    #[serde(default)]
    pub ingresses: Vec<IngressConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    pub r#type: String,
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub config: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressConfig {
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    #[serde(default)]
    pub paths: Vec<PathConfig>,
    #[serde(default)]
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    #[serde(default)]
    pub secret_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    pub path: String,
    #[serde(default = "default_path_type")]
    pub r#type: String,
    pub backend: String,
    #[serde(default)]
    pub service: Option<ServiceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub port: u16,
}

fn default_path_type() -> String {
    "prefix".to_string()
}

impl FileConfigProvider {
    async fn load_and_apply_config(
        &self,
        router: &::ksbh_core::routing::RouterWriter,
        _certs: &::ksbh_core::certs::CertsWriter,
    ) -> Result<(), Box<dyn ::std::error::Error>> {
        let config_content = ::std::fs::read_to_string(&self.config_path)?;
        let config: Config = ::serde_yaml::from_str(&config_content)?;

        for module in &config.modules {
            let mut module_config: ::hashbrown::HashMap<
                ::ksbh_types::KsbhStr,
                ::ksbh_types::KsbhStr,
            > = ::hashbrown::HashMap::new();

            for (key, value) in &module.config {
                let resolved_value = self.resolve_env_var(value);
                module_config.insert(
                    ::ksbh_types::KsbhStr::new(key),
                    ::ksbh_types::KsbhStr::new(&resolved_value),
                );
            }

            let module_type = match module.r#type.to_lowercase().as_str() {
                "ratelimit" | "rate_limit" | "rate-limit" | "rate limit" => {
                    ::ksbh_core::modules::ModuleConfigurationType::RateLimit
                }
                "httpstohttps" | "http_to_https" | "http-to-https" | "http2https" | "http to https" => {
                    ::ksbh_core::modules::ModuleConfigurationType::HttpToHttps
                }
                "robotstxt" | "robots_txt" | "robots.txt" | "robotsdottxt" => {
                    ::ksbh_core::modules::ModuleConfigurationType::RobotsDotTXT
                }
                "oidc" => ::ksbh_core::modules::ModuleConfigurationType::OIDC,
                "pow" | "proofofwork" | "proof-of-work" | "proof of work" => {
                    ::ksbh_core::modules::ModuleConfigurationType::POW
                }
                _ => {
                    tracing::warn!(
                        "Unknown module type '{}' - treating as Custom. Valid types: rate-limit, http-to-https, robots.txt, oidc, pow",
                        module.r#type
                    );
                    ::ksbh_core::modules::ModuleConfigurationType::Custom(module.r#type.clone())
                }
            };

            let spec = ::ksbh_core::modules::ModuleConfigurationSpec {
                name: module.name.clone(),
                r#type: module_type,
                global: module.global,
                requires_proper_request: true,
                secret_ref: None,
                requires_body: false,
            };

            router.upsert_module(
                &module.name,
                module.global,
                ::std::sync::Arc::new(module_config),
                spec,
            );

            tracing::info!("Loaded module: {}", module.name);
        }

        for ingress in &config.ingresses {
            let mut paths: Vec<(::std::sync::Arc<str>, ::ksbh_core::routing::HostPaths)> =
                ::std::vec::Vec::new();
            let mut host_paths = ::ksbh_core::routing::HostPaths::default();

            for path_config in &ingress.paths {
                let backend = match path_config.backend.to_lowercase().as_str() {
                    "service" => {
                        if let Some(ref svc) = path_config.service {
                            ::ksbh_core::routing::ServiceBackendType::ServiceBackend(
                                ::ksbh_core::routing::ServiceBackend {
                                    name: ::ksbh_types::KsbhStr::new(&svc.name),
                                    port: svc.port,
                                },
                            )
                        } else {
                            ::ksbh_core::routing::ServiceBackendType::None
                        }
                    }
                    "static" => ::ksbh_core::routing::ServiceBackendType::Static,
                    "self" => ::ksbh_core::routing::ServiceBackendType::ToSelf(None),
                    _ => ::ksbh_core::routing::ServiceBackendType::None,
                };

                let path_key = ::ksbh_types::KsbhStr::new(&path_config.path);
                match path_config.r#type.as_str() {
                    "exact" => {
                        host_paths.exact.insert(path_key, backend);
                    }
                    "prefix" => {
                        host_paths.prefix.push((path_key, backend));
                    }
                    _ => {
                        host_paths.implementation_specific.push((path_key, backend));
                    }
                }
            }

            if let Some(ref tls) = ingress.tls
                && let (Some(cert_file), Some(key_file)) = (&tls.cert_file, &tls.key_file)
            {
                tracing::warn!(
                    "TLS cert loading from files not yet implemented: cert_file={}, key_file={}",
                    cert_file,
                    key_file
                );
            }

            let module_names: Vec<::std::sync::Arc<str>> = ingress
                .modules
                .iter()
                .map(|s| ::std::sync::Arc::from(s.as_str()))
                .collect();

            paths.push((::std::sync::Arc::from(ingress.host.as_str()), host_paths));
            router.insert_ingress(&ingress.name, paths, module_names);

            tracing::info!(
                "Loaded ingress: {} for host: {}",
                ingress.name,
                ingress.host
            );
        }

        Ok(())
    }

    fn resolve_env_var(&self, value: &str) -> String {
        if let Some(var_name) = value.strip_prefix('$')
            && let Ok(env_value) = ::ksbh_core::utils::get_env_prefer_file(var_name)
        {
            return env_value;
        }
        value.to_string()
    }

    async fn watch_config_file(
        self: ::std::sync::Arc<Self>,
        router: ::ksbh_core::routing::RouterWriter,
        certs: ::ksbh_core::certs::CertsWriter,
        mut shutdown: ::tokio::sync::watch::Receiver<bool>,
    ) {
        use ::notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

        let (tx, mut rx) = ::tokio::sync::mpsc::channel(100);

        let mut watcher = match RecommendedWatcher::new(
            move |res: ::std::result::Result<::notify::Event, ::notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&self.config_path, RecursiveMode::NonRecursive) {
            tracing::error!("Failed to watch config file: {}", e);
            return;
        }

        tracing::info!("Watching config file: {:?}", self.config_path);

        loop {
            ::tokio::select! {
                _ = shutdown.changed() => {
                    tracing::info!("Config file watcher shutdown");
                    break;
                }
                Some(event) = rx.recv() => {
                    if let ::notify::EventKind::Modify(_) | ::notify::EventKind::Create(_) = event.kind {
                        tracing::info!("Config file changed, reloading...");
                        match self.load_and_apply_config(&router, &certs).await {
                            Ok(_) => tracing::info!("Config reloaded successfully"),
                            Err(e) => tracing::error!("Failed to reload config: {}", e),
                        }
                    }
                }
            }
        }
    }
}

#[::async_trait::async_trait]
impl ::ksbh_core::config_provider::ConfigProvider for FileConfigProvider {
    async fn start(
        &self,
        router: ::ksbh_core::routing::RouterWriter,
        certs: ::ksbh_core::certs::CertsWriter,
        shutdown: ::tokio::sync::watch::Receiver<bool>,
    ) {
        let self_arc = ::std::sync::Arc::new(Self {
            config_path: self.config_path.clone(),
        });

        if let Err(e) = self_arc.load_and_apply_config(&router, &certs).await {
            tracing::error!("Failed to load initial config: {}", e);
        }

        self_arc.watch_config_file(router, certs, shutdown).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_var_dollar_sign() {
        let provider = FileConfigProvider::new(::std::path::PathBuf::from("/tmp/test.yaml"));

        // Note: set_var is unsafe in multi-threaded context, but this is for testing
        unsafe { ::std::env::set_var("TEST_VAR", "test_value") };
        assert_eq!(provider.resolve_env_var("$TEST_VAR"), "test_value");
        unsafe { ::std::env::remove_var("TEST_VAR") };
    }

    #[test]
    fn test_resolve_env_var_no_dollar() {
        let provider = FileConfigProvider::new(::std::path::PathBuf::from("/tmp/test.yaml"));
        assert_eq!(provider.resolve_env_var("plain_value"), "plain_value");
    }

    #[test]
    fn test_config_parsing() {
        let yaml = r#"
modules:
  - name: rate-limit
    type: RateLimit
    global: true
    config:
      rate: "100"

ingresses:
  - name: main
    host: example.com
    paths:
      - path: /api
        type: prefix
        backend: service
        service:
          name: api
          port: 8080
"#;
        let config: Config = ::serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.modules.len(), 1);
        assert_eq!(config.ingresses.len(), 1);
        assert_eq!(config.modules[0].name, "rate-limit");
        assert_eq!(config.ingresses[0].host, "example.com");
    }

    #[test]
    fn test_config_parsing_minimal() {
        let yaml = r#"
modules: []
ingresses: []
"#;
        let config: Config = ::serde_yaml::from_str(yaml).unwrap();
        assert!(config.modules.is_empty());
        assert!(config.ingresses.is_empty());
    }
}
