use crate::certs::CertsWriter;
use crate::routing::RouterWriter;

/// Errors produced by a [`ConfigProvider`].
pub type ConfigProviderError = Box<dyn ::std::error::Error + Send + Sync + 'static>;

/// Source of ingress data for the proxy.
///
/// A provider is responsible for *watching* whatever it watches (filesystem
/// for the file adapter, Kubernetes API for the cluster adapter) and calling
/// [`crate::routing::IngressRecipe::emit`] on each change to push the new
/// state into the router and certs registries.
///
/// `emit` runs for the lifetime of the provider — the caller passes the
/// pingora `ShutdownWatch` so the provider can return cleanly when the
/// process is asked to stop.
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn emit(
        &self,
        router: &mut RouterWriter,
        certs: &mut CertsWriter,
        shutdown: pingora_core::server::ShutdownWatch,
    ) -> Result<(), ConfigProviderError>;
}

pub struct ConfigService {
    provider: Box<dyn ConfigProvider>,
    router: RouterWriter,
    certs: CertsWriter,
}

impl ConfigService {
    pub fn new(
        provider: Box<dyn ConfigProvider>,
        router: RouterWriter,
        certs: CertsWriter,
    ) -> Self {
        Self {
            provider,
            router,
            certs,
        }
    }
}

#[async_trait::async_trait]
impl pingora_core::services::background::BackgroundService for ConfigService {
    async fn start(&self, shutdown: pingora_core::server::ShutdownWatch) {
        let mut router = self.router.clone();
        let mut certs = self.certs.clone();
        if let Err(e) = self
            .provider
            .emit(&mut router, &mut certs, shutdown)
            .await
        {
            tracing::error!("Config provider emit failed: {e}");
        }
    }
}
