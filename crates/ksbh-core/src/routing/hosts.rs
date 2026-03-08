use super::ServiceBackendType;

#[derive(Debug, Default, Clone)]
pub struct HostRegistry {
    pub hosts: ::std::sync::Arc<
        hashbrown::HashMap<ksbh_types::KsbhStr, ::std::sync::Arc<HostConfiguration>>,
    >,
}

#[derive(Debug, Default, Clone)]
pub struct GlobalConfig {
    pub modules: Vec<ksbh_types::KsbhStr>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostConfiguration {
    pub modules: Vec<ksbh_types::KsbhStr>,
    pub(crate) paths: ::std::sync::Arc<HostPaths>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostPaths {
    pub exact: hashbrown::HashMap<ksbh_types::KsbhStr, ServiceBackendType>,
    pub prefix: Vec<(ksbh_types::KsbhStr, ServiceBackendType)>,
    pub implementation_specific: Vec<(ksbh_types::KsbhStr, ServiceBackendType)>,
}

impl HostRegistry {}

impl HostPaths {
    pub fn find(&self, request_path: &str) -> Option<&ServiceBackendType> {
        if let Some(backend) = self.exact.get(request_path) {
            return Some(backend);
        }

        fn path_prefix_match(prefix: &str, path: &str) -> bool {
            if prefix == "/" {
                return true;
            }

            if path == prefix {
                return true;
            }

            path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/')
        }

        for (path, service_backend) in &self.prefix {
            if path_prefix_match(path.as_str(), request_path) {
                return Some(service_backend);
            }
        }

        for (path, service_backend) in &self.implementation_specific {
            if request_path.starts_with(path.as_str()) {
                return Some(service_backend);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::service_backend::{ServiceBackend, ServiceBackendType};
    use ksbh_types::KsbhStr;

    #[test]
    fn test_host_registry_default() {
        let registry = HostRegistry::default();
        assert!(registry.hosts.is_empty());
    }

    #[test]
    fn test_host_registry_clone() {
        let registry1 = HostRegistry::default();
        let registry2 = registry1.clone();
        assert!(registry2.hosts.is_empty());
    }

    #[test]
    fn test_global_config_default() {
        let config = GlobalConfig::default();
        assert!(config.modules.is_empty());
    }

    #[test]
    fn test_global_config_with_modules() {
        let config = GlobalConfig {
            modules: vec![KsbhStr::new("module1"), KsbhStr::new("module2")],
        };
        assert_eq!(config.modules.len(), 2);
    }

    #[test]
    fn test_host_configuration_default() {
        let config = HostConfiguration::default();
        assert!(config.modules.is_empty());
    }

    #[test]
    fn test_host_configuration_clone() {
        let config1 = HostConfiguration {
            modules: vec![KsbhStr::new("mod1")],
            paths: ::std::sync::Arc::new(HostPaths::default()),
        };
        let config2 = config1.clone();
        assert_eq!(config1.modules, config2.modules);
    }

    #[test]
    fn test_host_paths_default() {
        let paths = HostPaths::default();
        assert!(paths.exact.is_empty());
        assert!(paths.prefix.is_empty());
        assert!(paths.implementation_specific.is_empty());
    }

    #[test]
    fn test_host_paths_find_exact_match() {
        let mut paths = HostPaths::default();
        paths
            .exact
            .insert(KsbhStr::new("/api"), ServiceBackendType::Static);

        let result = paths.find("/api");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &ServiceBackendType::Static);
    }

    #[test]
    fn test_host_paths_find_no_match() {
        let paths = HostPaths::default();

        let result = paths.find("/api");
        assert!(result.is_none());
    }

    #[test]
    fn test_host_paths_find_root_wildcard() {
        let mut paths = HostPaths::default();
        paths
            .prefix
            .push((KsbhStr::new("/"), ServiceBackendType::Static));

        let result = paths.find("/any/path");
        assert!(result.is_some());
    }

    #[test]
    fn test_host_paths_find_prefix_match() {
        let mut paths = HostPaths::default();
        paths.prefix.push((
            KsbhStr::new("/api"),
            ServiceBackendType::ServiceBackend(ServiceBackend {
                name: KsbhStr::new("api-service"),
                port: 8080,
            }),
        ));

        let result = paths.find("/api/users");
        assert!(result.is_some());
    }

    #[test]
    fn test_host_paths_find_prefix_no_match() {
        let mut paths = HostPaths::default();
        paths
            .prefix
            .push((KsbhStr::new("/api"), ServiceBackendType::Static));

        let result = paths.find("/other");
        assert!(result.is_none());
    }

    #[test]
    fn test_host_paths_find_implementation_specific() {
        let mut paths = HostPaths::default();
        paths
            .implementation_specific
            .push((KsbhStr::new("/custom"), ServiceBackendType::ToSelf(None)));

        let result = paths.find("/custom/path");
        assert!(result.is_some());
    }

    #[test]
    fn test_host_paths_exact_takes_precedence() {
        let mut paths = HostPaths::default();
        paths
            .exact
            .insert(KsbhStr::new("/api"), ServiceBackendType::Static);
        paths
            .prefix
            .push((KsbhStr::new("/api"), ServiceBackendType::ToSelf(None)));

        let result = paths.find("/api");
        assert!(result.is_some());
    }

    #[test]
    fn test_host_paths_prefix_takes_precedence_over_implementation() {
        let mut paths = HostPaths::default();
        paths
            .prefix
            .push((KsbhStr::new("/api"), ServiceBackendType::Static));
        paths
            .implementation_specific
            .push((KsbhStr::new("/api"), ServiceBackendType::ToSelf(None)));

        let result = paths.find("/api/users");
        assert!(result.is_some());
    }
}
