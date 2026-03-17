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
