use super::ServiceBackendType;

/// Registry of all configured hosts and their configurations.
#[derive(Debug, Default, Clone)]
pub struct HostRegistry {
    /// Map of host name to host configuration
    pub hosts: ::std::sync::Arc<
        hashbrown::HashMap<ksbh_types::KsbhStr, ::std::sync::Arc<HostConfiguration>>,
    >,
}

#[derive(Debug, Default, Clone)]
pub struct GlobalConfig {
    pub modules: Vec<ksbh_types::KsbhStr>,
}

/// Configuration for a specific host including its modules and paths.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostConfiguration {
    /// List of module names enabled for this host
    pub modules: Vec<ksbh_types::KsbhStr>,
    /// Path routing configuration for this host
    pub(crate) paths: ::std::sync::Arc<HostPaths>,
}

/// Path routing configuration for a host with support for exact, prefix, and implementation-specific matching.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostPaths {
    /// Exact path matches
    pub exact: hashbrown::HashMap<ksbh_types::KsbhStr, ServiceBackendType>,
    /// Prefix-based path matches
    pub prefix: Vec<(ksbh_types::KsbhStr, ServiceBackendType)>,
    /// Implementation-specific path matches
    pub implementation_specific: Vec<(ksbh_types::KsbhStr, ServiceBackendType)>,
}

impl HostRegistry {}

impl HostPaths {
    /// Finds the backend service for a given request path.
    /// Checks exact matches first, then prefix matches, then implementation-specific matches.
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
