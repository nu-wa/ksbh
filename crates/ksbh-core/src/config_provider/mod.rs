use crate::certs::CertsWriter;
use crate::routing::RouterWriter;

#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn start(
        &self,
        router: RouterWriter,
        certs: CertsWriter,
        shutdown: tokio::sync::watch::Receiver<bool>,
    );
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
impl pingora::services::background::BackgroundService for ConfigService {
    async fn start(&self, shutdown: pingora::server::ShutdownWatch) {
        self.provider
            .start(self.router.clone(), self.certs.clone(), shutdown)
            .await;
    }
}

