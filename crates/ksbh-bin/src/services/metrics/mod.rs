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

        // TODO: fix this very dirty hack i hate it
        let mut rx = self
            .receiver
            .lock()
            .expect("Could not start metrics")
            .take()
            .expect("Can only have one metrics receiver");

        let metrics_receive = self.metrics_w.clone();
        let metrics_cleanup = self.metrics_w.clone();

        let rx_task = tokio::spawn(async move {
            while let Some(req_metric) = rx.recv().await {
                metrics_receive.add_http_request(req_metric).await;
            }
        });

        let clean_task = metrics_cleanup
            .clean_http_requests(tokio::time::Duration::from_mins(5))
            .await;
        let _ = shutdown.changed().await;

        rx_task.abort();
        clean_task.abort();

        tracing::info!("Ended Metrics service");
    }
}
