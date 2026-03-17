#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ModuleInnerConfig {
    #[allow(dead_code)]
    spec: ::std::sync::Arc<super::ModuleConfigurationSpec>,
    #[allow(dead_code)]
    config_values: super::ModuleConfigurationValues,
    config_kv_slice: ::std::sync::Arc<Vec<super::abi::ModuleKvSlice>>,
}

#[derive(Debug)]
pub struct ModuleRegistry {
    inner: ksbh_types::ArcHashMap<ksbh_types::KsbhStr, ModuleInnerConfig>,
    global_configs: ksbh_types::ArcHashMap<ksbh_types::KsbhStr, ModuleInnerConfig>,
}

#[derive(Debug, Clone)]
pub struct ModuleRegistryReader {
    registry: ::std::sync::Arc<ModuleRegistry>,
}

#[derive(Debug, Clone)]
pub struct ModuleRegistryWriter {
    registry: ::std::sync::Arc<ModuleRegistry>,
}

impl ModuleInnerConfig {
    pub fn new(
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) -> Self {
        let mut entries = Vec::with_capacity(config.len());
        for (k, v) in config.iter() {
            entries.push(super::abi::ModuleKvSlice {
                key: bytes::Bytes::copy_from_slice(k.as_bytes()),
                value: bytes::Bytes::copy_from_slice(v.as_bytes()),
            });
        }
        Self {
            spec: ::std::sync::Arc::new(spec),
            config_values: config,
            config_kv_slice: ::std::sync::Arc::new(entries),
        }
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self {
            inner: ksbh_types::ArcHashMap::new(::std::sync::Arc::new(
                ::std::collections::HashMap::new(),
            )),
            global_configs: ksbh_types::ArcHashMap::new(::std::sync::Arc::new(
                ::std::collections::HashMap::new(),
            )),
        }
    }
}

impl ModuleRegistry {
    pub fn create() -> (ModuleRegistryReader, ModuleRegistryWriter) {
        let _self = ::std::sync::Arc::new(Self::default());

        (
            ModuleRegistryReader {
                registry: _self.clone(),
            },
            ModuleRegistryWriter { registry: _self },
        )
    }

    fn add_configuration(
        &self,
        name: &str,
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) {
        self.inner.rcu(move |old| {
            let mut new = ::std::collections::HashMap::clone(old);
            new.insert(
                ksbh_types::KsbhStr::new(name),
                ModuleInnerConfig::new(config.clone(), spec.clone()),
            );
            new
        });
    }

    fn add_global_configuration(
        &self,
        name: &str,
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) {
        self.global_configs.rcu(move |old| {
            let mut new = ::std::collections::HashMap::clone(old);
            new.insert(
                ksbh_types::KsbhStr::new(name),
                ModuleInnerConfig::new(config.clone(), spec.clone()),
            );
            new
        });
    }

    fn delete_config_by_name(&self, name: &str, global: bool) {
        let key = ksbh_types::KsbhStr::new(name);

        if global {
            self.global_configs.rcu(move |old| {
                let mut new = ::std::collections::HashMap::clone(old);
                new.remove(&key);
                new
            });
        } else {
            self.inner.rcu(move |old| {
                let mut new = ::std::collections::HashMap::clone(old);
                new.remove(&key);
                new
            });
        }
    }

    fn update_config_by_name(
        &self,
        name: &str,
        new_config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
        global: bool,
    ) {
        if global {
            self.global_configs.rcu(move |old| {
                let mut new = ::std::collections::HashMap::clone(old);
                new.insert(
                    ksbh_types::KsbhStr::new(name),
                    ModuleInnerConfig::new(new_config.clone(), spec.clone()),
                );
                new
            });
        } else {
            self.inner.rcu(move |old| {
                let mut new = ::std::collections::HashMap::clone(old);
                new.insert(
                    ksbh_types::KsbhStr::new(name),
                    ModuleInnerConfig::new(new_config.clone(), spec.clone()),
                );
                new
            });
        }
    }

    fn get_config(&self, name: &str) -> Option<crate::routing::request_match::RequestMatchModule> {
        let key = ksbh_types::KsbhStr::new(name);
        self.inner
            .load()
            .get(&key)
            .map(|cfg| crate::routing::request_match::RequestMatchModule {
                name: ::std::sync::Arc::new(key),
                mod_spec: cfg.spec.clone(),
                config_kv_slice: cfg.config_kv_slice.clone(),
            })
    }

    fn get_global_configs(&self) -> Vec<crate::routing::request_match::RequestMatchModule> {
        let global_configs = self.global_configs.load();
        let mut res = vec![];

        for (key, value) in global_configs.iter() {
            res.push(crate::routing::request_match::RequestMatchModule {
                name: ::std::sync::Arc::new(key.clone()),
                mod_spec: value.spec.clone(),
                config_kv_slice: value.config_kv_slice.clone(),
            });
        }

        res.sort_by(|a, b| {
            b.mod_spec
                .r#type
                .get_weight()
                .cmp(&a.mod_spec.r#type.get_weight())
        });

        res
    }
}

impl ModuleRegistryReader {
    pub fn get_config(
        &self,
        name: &str,
    ) -> Option<crate::routing::request_match::RequestMatchModule> {
        self.registry.get_config(name)
    }

    pub fn get_global_configs(&self) -> Vec<crate::routing::request_match::RequestMatchModule> {
        self.registry.get_global_configs()
    }
}

impl ModuleRegistryWriter {
    pub fn delete_config(&self, name: &str) {
        self.registry.delete_config_by_name(name, false);
    }

    pub fn delete_global_config(&self, name: &str) {
        self.registry.delete_config_by_name(name, true);
    }

    pub fn add_configuration(
        &self,
        name: &str,
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) {
        self.registry.add_configuration(name, config, spec);
    }

    pub fn add_global_configuration(
        &self,
        name: &str,
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) {
        self.registry.add_global_configuration(name, config, spec);
    }

    pub fn get_config(
        &self,
        name: &str,
    ) -> Option<crate::routing::request_match::RequestMatchModule> {
        self.registry.get_config(name)
    }

    pub fn update_config(
        &self,
        name: &str,
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) {
        self.registry
            .update_config_by_name(name, config, spec, false);
    }

    pub fn update_global_config(
        &self,
        name: &str,
        config: super::ModuleConfigurationValues,
        spec: super::ModuleConfigurationSpec,
    ) {
        self.registry
            .update_config_by_name(name, config, spec, true);
    }

    pub fn get_global_configs(&self) -> Vec<crate::routing::request_match::RequestMatchModule> {
        self.registry.get_global_configs()
    }
}

impl From<&::std::sync::Arc<ModuleRegistry>> for ModuleRegistryReader {
    fn from(value: &::std::sync::Arc<ModuleRegistry>) -> Self {
        Self {
            registry: ::std::sync::Arc::clone(value),
        }
    }
}

impl From<&::std::sync::Arc<ModuleRegistry>> for ModuleRegistryWriter {
    fn from(value: &::std::sync::Arc<ModuleRegistry>) -> Self {
        Self {
            registry: ::std::sync::Arc::clone(value),
        }
    }
}
