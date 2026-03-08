#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceBackendType {
    ServiceBackend(ServiceBackend),
    ToSelf(Option<ksbh_types::KsbhStr>),
    Static,
    Error(&'static str),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceBackend {
    pub name: ksbh_types::KsbhStr,
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksbh_types::KsbhStr;

    #[test]
    fn test_service_backend_type_service_backend() {
        let backend = ServiceBackendType::ServiceBackend(ServiceBackend {
            name: KsbhStr::new("my-service"),
            port: 8080,
        });
        assert!(matches!(backend, ServiceBackendType::ServiceBackend(_)));
    }

    #[test]
    fn test_service_backend_type_to_self() {
        let backend = ServiceBackendType::ToSelf(None);
        assert!(matches!(backend, ServiceBackendType::ToSelf(None)));

        let backend_with_name = ServiceBackendType::ToSelf(Some(KsbhStr::new("app1")));
        assert!(matches!(
            backend_with_name,
            ServiceBackendType::ToSelf(Some(_))
        ));
    }

    #[test]
    fn test_service_backend_type_static() {
        let backend = ServiceBackendType::Static;
        assert_eq!(backend, ServiceBackendType::Static);
    }

    #[test]
    fn test_service_backend_type_error() {
        let backend = ServiceBackendType::Error("Something went wrong");
        assert!(matches!(backend, ServiceBackendType::Error(_)));
    }

    #[test]
    fn test_service_backend_type_none() {
        let backend = ServiceBackendType::None;
        assert_eq!(backend, ServiceBackendType::None);
    }

    #[test]
    fn test_service_backend_type_clone() {
        let backend1 = ServiceBackendType::ServiceBackend(ServiceBackend {
            name: KsbhStr::new("test"),
            port: 3000,
        });
        let backend2 = backend1.clone();
        assert_eq!(backend1, backend2);
    }

    #[test]
    fn test_service_backend_type_debug() {
        let backend = ServiceBackendType::Static;
        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("Static"));
    }

    #[test]
    fn test_service_backend_new() {
        let backend = ServiceBackend {
            name: KsbhStr::new("my-service"),
            port: 8080,
        };
        assert_eq!(backend.name.as_str(), "my-service");
        assert_eq!(backend.port, 8080);
    }

    #[test]
    fn test_service_backend_clone() {
        let backend1 = ServiceBackend {
            name: KsbhStr::new("test"),
            port: 3000,
        };
        let backend2 = backend1.clone();
        assert_eq!(backend1, backend2);
    }

    #[test]
    fn test_service_backend_debug() {
        let backend = ServiceBackend {
            name: KsbhStr::new("test"),
            port: 3000,
        };
        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_service_backend_type_ord() {
        let mut backends = vec![
            ServiceBackendType::None,
            ServiceBackendType::Static,
            ServiceBackendType::Error("err"),
        ];
        backends.sort();
    }
}
