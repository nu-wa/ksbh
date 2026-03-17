pub struct MetricsService {
    receiver:
        ::std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<ksbh_core::metrics::RequestMetrics>>>,
    metrics_w: ksbh_core::metrics::MetricsWriter,
}

impl MetricsService {
    pub fn new(
        receiver: tokio::sync::mpsc::Receiver<ksbh_core::metrics::RequestMetrics>,
        metrics_w: ksbh_core::metrics::MetricsWriter,
    ) -> Self {
        Self {
            receiver: ::std::sync::Mutex::new(Some(receiver)),
            metrics_w,
        }
    }
}

#[async_trait::async_trait]
impl pingora::services::background::BackgroundService for MetricsService {
    async fn start(&self, mut shutdown: pingora::server::ShutdownWatch) {
        tracing::info!("Starting Metrics service");

        let mut rx = self
            .receiver
            .lock()
            .expect("Could not start metrics")
            .take()
            .expect("Can only have one metrics receiver");

        let metrics_w = self.metrics_w.clone();

        let rx_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = rx.recv() => {
                        match result {
                            Some(req_metric) => {
                                metrics_w.log_request(req_metric).await;
                            }
                            None => {
                                tracing::warn!("Metrics channel closed, stopping");
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {}
                }
            }
        });

        let _ = shutdown.changed().await;

        rx_task.abort();

        tracing::info!("Ended Metrics service");
    }
}
