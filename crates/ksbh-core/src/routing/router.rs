#[derive(Debug, Clone)]
pub struct ModuleInnerConfig {
    pub spec: ::std::sync::Arc<crate::modules::ModuleConfigurationSpec>,
    pub config_values: crate::modules::ModuleConfigurationValues,
    pub config_kv_slice: ::std::sync::Arc<Vec<crate::modules::abi::ModuleKvSlice>>,
}

#[derive(Debug, Clone, Default)]
pub struct IngressModuleConfig {
    pub modules: Vec<::std::sync::Arc<str>>,
    pub excluded_modules: Vec<::std::sync::Arc<str>>,
}

#[derive(Debug, Clone)]
pub struct RuntimeIngressSnapshot {
    pub ingress_name: ::std::string::String,
    pub host: ::std::string::String,
    pub attached_modules: Vec<::std::string::String>,
    pub excluded_modules: Vec<::std::string::String>,
    pub merged_modules: Vec<::std::string::String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeModuleSnapshot {
    pub name: ::std::string::String,
    pub module_type: crate::modules::ModuleConfigurationType,
    pub global: bool,
    pub config_key_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeStateSnapshot {
    pub ingresses: Vec<RuntimeIngressSnapshot>,
    pub modules: Vec<RuntimeModuleSnapshot>,
}

#[derive(Debug)]
pub struct Router {
    hosts: scc::HashMap<ksbh_types::KsbhStr, ::std::sync::Arc<Host>>,
    module_registry: scc::HashMap<ksbh_types::KsbhStr, ::std::sync::Arc<ModuleInnerConfig>>,
    global_module_registry: scc::HashMap<ksbh_types::KsbhStr, ::std::sync::Arc<ModuleInnerConfig>>,
    ingress_module_config: scc::HashMap<::std::sync::Arc<str>, IngressModuleConfig>,
}

#[derive(Debug, Clone)]
pub struct RouterReader {
    router: ::std::sync::Arc<Router>,
}

#[derive(Debug, Clone)]
pub struct RouterWriter {
    router: ::std::sync::Arc<Router>,
}

#[derive(Debug, Clone)]
pub(super) struct Ingress {
    name: ::std::sync::Arc<str>,
    pub merged_modules: ::std::sync::Arc<Vec<super::request_match::RequestMatchModule>>,
}

#[derive(Debug, Clone)]
struct HostEntry {
    ingress: ::std::sync::Arc<Ingress>,
    paths: super::HostPaths,
}

#[derive(Debug, Clone)]
pub struct Host {
    entries: Vec<HostEntry>,
}

impl Router {
    fn compare_request_match_modules(
        a: &super::request_match::RequestMatchModule,
        b: &super::request_match::RequestMatchModule,
    ) -> ::std::cmp::Ordering {
        b.mod_spec
            .weight
            .cmp(&a.mod_spec.weight)
            .then_with(|| a.name.as_ref().cmp(b.name.as_ref()))
    }

    fn sort_request_match_modules(modules: &mut [super::request_match::RequestMatchModule]) {
        modules.sort_by(Self::compare_request_match_modules);
    }

    fn build_merged_ingress_modules(
        &self,
        ingress_name: &::std::sync::Arc<str>,
        module_config: &IngressModuleConfig,
        global_modules: &[super::request_match::RequestMatchModule],
        module_definitions: &::std::collections::HashMap<
            ksbh_types::KsbhStr,
            ::std::sync::Arc<ModuleInnerConfig>,
        >,
    ) -> Vec<super::request_match::RequestMatchModule> {
        let excluded = module_config.excluded_modules.clone();
        let route_module_names: Vec<&str> =
            module_config.modules.iter().map(|s| s.as_ref()).collect();

        let mut result: Vec<super::request_match::RequestMatchModule> = global_modules
            .iter()
            .filter(|m| {
                let name = m.name.as_ref();
                if excluded.iter().any(|e| e.as_ref() == name) {
                    if self
                        .global_module_registry
                        .get_sync(&ksbh_types::KsbhStr::new(name))
                        .is_none()
                    {
                        tracing::warn!(
                            "Excluded module '{}' not found in global modules for ingress '{}'",
                            name,
                            ingress_name
                        );
                    }
                    false
                } else {
                    !route_module_names.iter().any(|n| *n == name)
                }
            })
            .cloned()
            .collect();

        let mut ingress_modules = Vec::new();

        for module_name in &module_config.modules {
            let key = ksbh_types::KsbhStr::new(module_name.as_ref());

            if let Some(def) = module_definitions.get(&key) {
                ingress_modules.push(super::request_match::RequestMatchModule {
                    name: ::std::sync::Arc::new(key.clone()),
                    mod_spec: def.spec.clone(),
                    config_kv_slice: def.config_kv_slice.clone(),
                });
            }
        }

        Self::sort_request_match_modules(&mut ingress_modules);
        result.extend(ingress_modules);

        result
    }

    /// Creates a new RouterReader/RouterWriter pair for concurrent access.
    pub fn create() -> (RouterReader, RouterWriter) {
        let _self = ::std::sync::Arc::new(Router::default());

        (
            RouterReader {
                router: _self.clone(),
            },
            RouterWriter { router: _self },
        )
    }

    fn find_route(
        &self,
        request: &ksbh_types::requests::http_request::HttpRequest,
    ) -> Option<super::request_match::RequestMatch> {
        let path = if request.query.path.len() > 1 && request.query.path.ends_with('/') {
            request.query.path.trim_end_matches('/')
        } else {
            request.query.path.as_str()
        };
        let key = ksbh_types::KsbhStr::new(request.host.as_str());

        self.hosts
            .read_sync(&key, |_, host| {
                for entry in &host.entries {
                    if let Some(backend) = entry.paths.find(path) {
                        return Some(super::RequestMatch {
                            backend: backend.clone(),
                            modules: (*entry.ingress.merged_modules).clone(),
                        });
                    }
                }
                None
            })
            .flatten()
    }

    fn upsert_module(
        &self,
        name: &str,
        global: bool,
        config: crate::modules::ModuleConfigurationValues,
        spec: crate::modules::ModuleConfigurationSpec,
    ) {
        let key = ksbh_types::KsbhStr::new(name);
        let mut entries = Vec::with_capacity(config.len());
        for (k, v) in config.iter() {
            entries.push(crate::modules::abi::ModuleKvSlice {
                key: bytes::Bytes::copy_from_slice(k.as_bytes()),
                value: bytes::Bytes::copy_from_slice(v.as_bytes()),
            });
        }
        let inner = ::std::sync::Arc::new(ModuleInnerConfig {
            spec: ::std::sync::Arc::new(spec),
            config_values: config,
            config_kv_slice: ::std::sync::Arc::new(entries),
        });

        if global {
            self.global_module_registry.upsert_sync(key, inner);
        } else {
            self.module_registry.upsert_sync(key, inner);
        }

        self.record_runtime_state_update("module", "upsert");
        self.reload_ingresses();
    }

    fn delete_module_config(&self, name: &str) {
        let key = ksbh_types::KsbhStr::new(name);

        self.module_registry.remove_sync(&key);
        self.global_module_registry.remove_sync(&key);

        self.record_runtime_state_update("module", "delete");
        self.reload_ingresses();
    }

    fn insert_ingress(
        &self,
        ingress_name: &str,
        hosts: Vec<(::std::sync::Arc<str>, super::HostPaths)>,
        module_config: IngressModuleConfig,
    ) {
        let ingress_name: ::std::sync::Arc<str> = ::std::sync::Arc::from(ingress_name);

        self.ingress_module_config
            .upsert_sync(ingress_name.clone(), module_config);

        let merged_modules = ::std::sync::Arc::new(self.get_ingress_modules(&ingress_name));
        let ingress = ::std::sync::Arc::new(Ingress {
            name: ingress_name,
            merged_modules,
        });

        for (host_name, paths) in hosts {
            let host_key = ksbh_types::KsbhStr::new(host_name.as_ref());
            let new_entry = HostEntry {
                ingress: ingress.clone(),
                paths,
            };

            let existing = self
                .hosts
                .read_sync(&host_key, |_, host| host.entries.clone());

            let mut entries = existing.unwrap_or_default();
            entries.push(new_entry);

            self.hosts.upsert_sync(
                ksbh_types::KsbhStr::new(host_name),
                ::std::sync::Arc::new(Host { entries }),
            );
        }

        self.record_runtime_state_update("ingress", "upsert");
        self.refresh_runtime_metrics();
    }

    fn delete_ingress(&self, ingress_name: &str) {
        let ingress_name = ::std::sync::Arc::from(ingress_name);

        self.ingress_module_config.remove_sync(&ingress_name);

        let mut keys: Vec<ksbh_types::KsbhStr> = Vec::new();
        let mut entry = self.hosts.begin_sync();

        while let Some(occupied_entry) = entry {
            keys.push(occupied_entry.key().clone());

            entry = occupied_entry.next_sync();
        }

        for key in keys {
            let entries = match self.hosts.read_sync(&key, |_, host| host.entries.clone()) {
                Some(e) => e,
                None => continue,
            };

            let before = entries.len();
            let new_entries: Vec<_> = entries
                .into_iter()
                .filter(|e| e.ingress.name != ingress_name)
                .collect();

            if new_entries.len() == before {
                continue;
            }

            if new_entries.is_empty() {
                self.hosts.remove_sync(&key);
            } else {
                self.hosts.upsert_sync(
                    key,
                    ::std::sync::Arc::new(Host {
                        entries: new_entries,
                    }),
                );
            }
        }

        self.record_runtime_state_update("ingress", "delete");
        self.refresh_runtime_metrics();
    }

    fn get_ingress_modules(
        &self,
        ingress_name: &::std::sync::Arc<str>,
    ) -> Vec<super::request_match::RequestMatchModule> {
        let module_config = self
            .ingress_module_config
            .read_sync(ingress_name, |_, v| v.clone())
            .unwrap_or_default();

        let global_modules = self.get_global_modules();
        let mut module_definitions: ::std::collections::HashMap<
            ksbh_types::KsbhStr,
            ::std::sync::Arc<ModuleInnerConfig>,
        > = ::std::collections::HashMap::new();
        let mut entry = self.module_registry.begin_sync();

        while let Some(occupied_entry) = entry {
            module_definitions.insert(occupied_entry.key().clone(), occupied_entry.get().clone());
            entry = occupied_entry.next_sync();
        }

        self.build_merged_ingress_modules(
            ingress_name,
            &module_config,
            &global_modules,
            &module_definitions,
        )
    }

    fn reload_ingresses(&self) {
        let global_modules = self.get_global_modules();

        let mut module_definitions: ::std::collections::HashMap<
            ksbh_types::KsbhStr,
            ::std::sync::Arc<ModuleInnerConfig>,
        > = ::std::collections::HashMap::new();
        let mut entry = self.module_registry.begin_sync();

        while let Some(occupied_entry) = entry {
            module_definitions.insert(occupied_entry.key().clone(), occupied_entry.get().clone());

            entry = occupied_entry.next_sync();
        }

        let mut ingress_configs: Vec<(::std::sync::Arc<str>, IngressModuleConfig)> = Vec::new();
        let mut entry = self.ingress_module_config.begin_sync();

        while let Some(occupied_entry) = entry {
            ingress_configs.push((occupied_entry.key().clone(), occupied_entry.get().clone()));

            entry = occupied_entry.next_sync();
        }

        let mut ingress_modules: ::std::collections::HashMap<
            ::std::sync::Arc<str>,
            ::std::sync::Arc<Vec<super::request_match::RequestMatchModule>>,
        > = ::std::collections::HashMap::new();

        for (ingress_name, module_config) in ingress_configs {
            let list = self.build_merged_ingress_modules(
                &ingress_name,
                &module_config,
                &global_modules,
                &module_definitions,
            );

            ingress_modules.insert(ingress_name, ::std::sync::Arc::new(list));
        }

        let mut host_keys: Vec<ksbh_types::KsbhStr> = Vec::new();
        let mut entry = self.hosts.begin_sync();

        while let Some(occupied_entry) = entry {
            host_keys.push(occupied_entry.key().clone());

            entry = occupied_entry.next_sync();
        }

        for key in host_keys {
            let entries = match self.hosts.read_sync(&key, |_, host| host.entries.clone()) {
                Some(e) => e,
                None => continue,
            };

            let new_entries: Vec<HostEntry> = entries
                .into_iter()
                .map(|entry| {
                    let merged = ingress_modules
                        .get(&entry.ingress.name)
                        .cloned()
                        .unwrap_or_else(|| ::std::sync::Arc::new(global_modules.clone()));

                    HostEntry {
                        ingress: ::std::sync::Arc::new(Ingress {
                            name: entry.ingress.name.clone(),
                            merged_modules: merged,
                        }),
                        paths: entry.paths,
                    }
                })
                .collect();

            self.hosts.upsert_sync(
                key,
                ::std::sync::Arc::new(Host {
                    entries: new_entries,
                }),
            );
        }

        self.record_runtime_state_update("ingress", "reload");
        self.refresh_runtime_metrics();
    }

    fn get_global_modules(&self) -> Vec<super::request_match::RequestMatchModule> {
        let mut result = Vec::new();
        let mut entry = self.global_module_registry.begin_sync();

        while let Some(occupied_entry) = entry {
            let inner = occupied_entry.get();
            result.push(super::request_match::RequestMatchModule {
                name: ::std::sync::Arc::new(occupied_entry.key().clone()),
                mod_spec: inner.spec.clone(),
                config_kv_slice: inner.config_kv_slice.clone(),
            });

            entry = occupied_entry.next_sync();
        }

        Self::sort_request_match_modules(&mut result);

        result
    }

    fn snapshot_runtime_state(&self) -> RuntimeStateSnapshot {
        let mut snapshot = RuntimeStateSnapshot::default();

        let mut module_entry = self.global_module_registry.begin_sync();
        while let Some(occupied_entry) = module_entry {
            let inner = occupied_entry.get();
            snapshot.modules.push(RuntimeModuleSnapshot {
                name: occupied_entry.key().to_string(),
                module_type: inner.spec.r#type.clone(),
                global: true,
                config_key_count: inner.config_values.len(),
            });

            module_entry = occupied_entry.next_sync();
        }

        let mut module_entry = self.module_registry.begin_sync();
        while let Some(occupied_entry) = module_entry {
            let inner = occupied_entry.get();
            snapshot.modules.push(RuntimeModuleSnapshot {
                name: occupied_entry.key().to_string(),
                module_type: inner.spec.r#type.clone(),
                global: false,
                config_key_count: inner.config_values.len(),
            });

            module_entry = occupied_entry.next_sync();
        }

        let mut host_entry = self.hosts.begin_sync();
        while let Some(occupied_entry) = host_entry {
            let host_name = occupied_entry.key().to_string();
            let host = occupied_entry.get();

            for entry in &host.entries {
                let ingress_name = entry.ingress.name.to_string();
                let ingress_config = self
                    .ingress_module_config
                    .read_sync(&entry.ingress.name, |_, value| value.clone())
                    .unwrap_or_default();

                snapshot.ingresses.push(RuntimeIngressSnapshot {
                    ingress_name,
                    host: host_name.clone(),
                    attached_modules: ingress_config
                        .modules
                        .iter()
                        .map(|name| name.to_string())
                        .collect(),
                    excluded_modules: ingress_config
                        .excluded_modules
                        .iter()
                        .map(|name| name.to_string())
                        .collect(),
                    merged_modules: entry
                        .ingress
                        .merged_modules
                        .iter()
                        .map(|module| module.name.to_string())
                        .collect(),
                });
            }

            host_entry = occupied_entry.next_sync();
        }

        snapshot
    }

    fn record_runtime_state_update(&self, kind: &str, action: &str) {
        crate::metrics::prom::RUNTIME_STATE_UPDATES_TOTAL
            .with_label_values(&[kind, action])
            .inc();
    }

    fn refresh_runtime_metrics(&self) {
        crate::metrics::runtime_state::observe_runtime_snapshot(&self.snapshot_runtime_state());
    }
}

impl RouterReader {
    /// Main routing decision method. Finds the best matching route for an HTTP request
    /// based on host and path, returning the backend service and attached modules.
    pub fn find_route(
        &self,
        http_request: &ksbh_types::requests::http_request::HttpRequest,
    ) -> Option<super::request_match::RequestMatch> {
        self.router.find_route(http_request)
    }

    /// Returns the list of global module configurations that apply to all ingresses.
    pub fn get_global_modules_configs(&self) -> Vec<super::request_match::RequestMatchModule> {
        self.router.get_global_modules()
    }

    pub fn snapshot_runtime_state(&self) -> RuntimeStateSnapshot {
        self.router.snapshot_runtime_state()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self {
            hosts: scc::HashMap::new(),
            module_registry: scc::HashMap::new(),
            global_module_registry: scc::HashMap::new(),
            ingress_module_config: scc::HashMap::new(),
        }
    }
}

impl From<&::std::sync::Arc<Router>> for RouterReader {
    fn from(value: &::std::sync::Arc<Router>) -> Self {
        Self {
            router: ::std::sync::Arc::clone(value),
        }
    }
}

impl From<&::std::sync::Arc<Router>> for RouterWriter {
    fn from(value: &::std::sync::Arc<Router>) -> Self {
        Self {
            router: ::std::sync::Arc::clone(value),
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    #[test]
    fn runtime_snapshot_tracks_modules_and_ingresses() {
        let (reader, writer) = super::Router::create();

        let mut module_config = hashbrown::HashMap::new();
        module_config.insert(
            ksbh_types::KsbhStr::new("content"),
            ksbh_types::KsbhStr::new("hello"),
        );

        writer.upsert_module(
            "robots-test",
            false,
            ::std::sync::Arc::new(module_config),
            crate::modules::ModuleConfigurationSpec {
                name: "robots-test".to_string(),
                r#type: crate::modules::ModuleConfigurationType::RobotsDotTXT,
                weight: 100,
                global: false,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );

        let mut host_paths = crate::routing::HostPaths::default();
        host_paths.prefix.push((
            ksbh_types::KsbhStr::new("/"),
            crate::routing::ServiceBackendType::Static,
        ));

        writer.insert_ingress(
            "ingress-a",
            vec![(::std::sync::Arc::from("example.local"), host_paths)],
            super::IngressModuleConfig {
                modules: vec![::std::sync::Arc::from("robots-test")],
                excluded_modules: Vec::new(),
            },
        );

        let snapshot = reader.snapshot_runtime_state();

        assert_eq!(snapshot.modules.len(), 1);
        assert_eq!(snapshot.modules[0].name, "robots-test");
        assert_eq!(snapshot.ingresses.len(), 1);
        assert_eq!(snapshot.ingresses[0].ingress_name, "ingress-a");
        assert_eq!(snapshot.ingresses[0].host, "example.local");
        assert_eq!(snapshot.ingresses[0].attached_modules, vec!["robots-test"]);
        assert_eq!(snapshot.ingresses[0].merged_modules, vec!["robots-test"]);
    }

    #[test]
    fn merged_modules_keep_global_and_ingress_scopes_separate() {
        let (_reader, writer) = super::Router::create();

        writer.upsert_module(
            "global-low",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "global-low".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("global-low".to_string()),
                weight: 10,
                global: true,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "global-high",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "global-high".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("global-high".to_string()),
                weight: 100,
                global: true,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "ingress-high",
            false,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "ingress-high".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("ingress-high".to_string()),
                weight: 1000,
                global: false,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "ingress-low",
            false,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "ingress-low".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("ingress-low".to_string()),
                weight: 1,
                global: false,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );

        let mut host_paths = crate::routing::HostPaths::default();
        host_paths.prefix.push((
            ksbh_types::KsbhStr::new("/"),
            crate::routing::ServiceBackendType::Static,
        ));

        writer.insert_ingress(
            "ingress-a",
            vec![(::std::sync::Arc::from("example.local"), host_paths)],
            super::IngressModuleConfig {
                modules: vec![
                    ::std::sync::Arc::from("ingress-low"),
                    ::std::sync::Arc::from("ingress-high"),
                ],
                excluded_modules: Vec::new(),
            },
        );

        let modules = writer
            .router
            .get_ingress_modules(&::std::sync::Arc::from("ingress-a"));
        let module_names: Vec<_> = modules
            .iter()
            .map(|module| module.name.to_string())
            .collect();

        assert_eq!(
            module_names,
            vec!["global-high", "global-low", "ingress-high", "ingress-low"]
        );
    }

    #[test]
    fn equal_weights_are_tiebroken_by_name() {
        let (_reader, writer) = super::Router::create();

        writer.upsert_module(
            "beta",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "beta".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("beta".to_string()),
                weight: 5,
                global: true,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "alpha",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "alpha".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("alpha".to_string()),
                weight: 5,
                global: true,
                requires_proper_request: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );

        let modules = writer.router.get_global_modules();
        let module_names: Vec<_> = modules
            .iter()
            .map(|module| module.name.to_string())
            .collect();

        assert_eq!(module_names, vec!["alpha", "beta"]);
    }
}

impl RouterWriter {
    pub fn delete_module_config(&self, name: &str) {
        self.router.delete_module_config(name);
    }

    /// Registers or updates a module configuration.
    /// If `global` is true, the module applies to all ingresses.
    pub fn upsert_module(
        &self,
        name: &str,
        global: bool,
        config: crate::modules::ModuleConfigurationValues,
        spec: crate::modules::ModuleConfigurationSpec,
    ) {
        self.router.upsert_module(name, global, config, spec);
    }

    /// Registers an ingress with its associated hosts and path configurations.
    pub fn insert_ingress(
        &self,
        ingress_name: &str,
        hosts: Vec<(::std::sync::Arc<str>, super::HostPaths)>,
        module_config: IngressModuleConfig,
    ) {
        self.router
            .insert_ingress(ingress_name, hosts, module_config);
    }

    pub fn delete_ingress(&self, ingress_name: &str) {
        self.router.delete_ingress(ingress_name);
    }

    pub fn reload_ingresses(&self) {
        self.router.reload_ingresses();
    }
}
