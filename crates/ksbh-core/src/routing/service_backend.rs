/// Backend routing destination type for service requests.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceBackendType {
    /// Direct service backend with name and port
    ServiceBackend(ServiceBackend),
    /// Route back to self, optionally with a specific path
    ToSelf(Option<ksbh_types::KsbhStr>),
    /// Static content backend
    Static,
    /// Error response with static message
    Error(&'static str),
    /// No backend configured
    None,
}

/// Backend service endpoint definition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceBackend {
    /// Service name identifier
    pub name: ksbh_types::KsbhStr,
    /// Port number
    pub port: u16,
}
