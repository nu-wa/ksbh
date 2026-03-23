pub mod hosts;
pub mod path_type;
pub mod request_match;
pub mod router;
pub mod service_backend;

pub use hosts::{GlobalConfig, HostConfiguration, HostPaths, HostRegistry};
pub use path_type::PathType;
pub use request_match::RequestMatch;
pub use router::{
    IngressModuleConfig, Router, RouterReader, RouterWriter, RuntimeIngressSnapshot,
    RuntimeModuleSnapshot, RuntimeStateSnapshot,
};
pub use service_backend::{ServiceBackend, ServiceBackendType};

#[async_trait::async_trait]
pub trait RoutingProvier {
    async fn start(
        &self,
        hosts: RouterWriter,
        modules: crate::modules::registry::ModuleRegistryWriter,
        certs: crate::certs::CertsWriter,
        client: kube::Client,
        shutdown: tokio::sync::watch::Receiver<bool>,
    );
}
