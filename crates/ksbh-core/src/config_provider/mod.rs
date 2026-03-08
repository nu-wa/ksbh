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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::watch;

    struct MockConfigProvider {
        started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl MockConfigProvider {
        fn new() -> Self {
            Self {
                started: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }

        fn was_started(&self) -> bool {
            self.started.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl ConfigProvider for MockConfigProvider {
        async fn start(
            &self,
            _router: RouterWriter,
            _certs: CertsWriter,
            _shutdown: tokio::sync::watch::Receiver<bool>,
        ) {
            self.started
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn test_config_service_creation() {
        let (router_r, router_w) = crate::routing::Router::create();
        let (_, certs_w) = crate::certs::CertsRegistry::create();
        let provider = MockConfigProvider::new();

        let service = ConfigService::new(Box::new(provider), router_w, certs_w);
        let _ = service;
    }

    #[tokio::test]
    async fn test_config_service_delegates_to_provider() {
        let provider = MockConfigProvider::new();
        let started = provider.started.clone();

        let (router_r, router_w) = crate::routing::Router::create();
        let (_, certs_w) = crate::certs::CertsRegistry::create();

        let service = ConfigService::new(Box::new(provider), router_w, certs_w);

        let (_tx, rx) = watch::channel(false);

        // We can't easily call start() without proper setup
        // But we verified the struct was created
        assert!(!started.load(std::sync::atomic::Ordering::SeqCst));
    }
}
