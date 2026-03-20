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

        self.reload_ingresses();
    }

    fn delete_module_config(&self, name: &str) {
        let key = ksbh_types::KsbhStr::new(name);

        self.module_registry.remove_sync(&key);
        self.global_module_registry.remove_sync(&key);

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
    }

    fn get_ingress_modules(
        &self,
        ingress_name: &::std::sync::Arc<str>,
    ) -> Vec<super::request_match::RequestMatchModule> {
        let module_config = self
            .ingress_module_config
            .read_sync(ingress_name, |_, v| v.clone())
            .unwrap_or_default();

        let excluded = module_config.excluded_modules.clone();
        let route_module_names: Vec<&str> =
            module_config.modules.iter().map(|s| s.as_ref()).collect();

        let global_modules = self.get_global_modules();
        let mut result: Vec<super::request_match::RequestMatchModule> = global_modules
            .into_iter()
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
            .collect();

        for n in &module_config.modules {
            let key = ksbh_types::KsbhStr::new(n.as_ref());

            if let Some(def) = self.module_registry.get_sync(&key) {
                result.push(super::request_match::RequestMatchModule {
                    name: ::std::sync::Arc::new(def.key().clone()),
                    mod_spec: def.spec.clone(),
                    config_kv_slice: def.config_kv_slice.clone(),
                });
            }
        }

        result.sort_by(|a, b| {
            b.mod_spec
                .r#type
                .get_weight()
                .cmp(&a.mod_spec.r#type.get_weight())
        });

        result
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
            let excluded = module_config.excluded_modules.clone();
            let route_module_names: Vec<&str> =
                module_config.modules.iter().map(|s| s.as_ref()).collect();

            let mut list: Vec<super::request_match::RequestMatchModule> = global_modules
                .clone()
                .into_iter()
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
                .collect();

            for n in &module_config.modules {
                let k = ksbh_types::KsbhStr::new(n.as_ref());

                if let Some(def) = module_definitions.get(&k) {
                    list.push(super::request_match::RequestMatchModule {
                        name: ::std::sync::Arc::new(k.clone()),
                        mod_spec: def.spec.clone(),
                        config_kv_slice: def.config_kv_slice.clone(),
                    });
                }
            }

            list.sort_by(|a, b| {
                b.mod_spec
                    .r#type
                    .get_weight()
                    .cmp(&a.mod_spec.r#type.get_weight())
            });

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

        result.sort_by(|a, b| {
            b.mod_spec
                .r#type
                .get_weight()
                .cmp(&a.mod_spec.r#type.get_weight())
        });

        result
    }
}

impl RouterReader {
    pub fn find_route(
        &self,
        http_request: &ksbh_types::requests::http_request::HttpRequest,
    ) -> Option<super::request_match::RequestMatch> {
        self.router.find_route(http_request)
    }

    pub fn get_global_modules_configs(&self) -> Vec<super::request_match::RequestMatchModule> {
        self.router.get_global_modules()
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

impl RouterWriter {
    pub fn delete_module_config(&self, name: &str) {
        self.router.delete_module_config(name);
    }

    pub fn upsert_module(
        &self,
        name: &str,
        global: bool,
        config: crate::modules::ModuleConfigurationValues,
        spec: crate::modules::ModuleConfigurationSpec,
    ) {
        self.router.upsert_module(name, global, config, spec);
    }

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
