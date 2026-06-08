//! Everything related to metrics

pub mod module_metric;
pub mod prom;
pub mod runtime_signals;
pub mod runtime_state;

pub use prometheus;

// Request data for metrics
#[derive(Clone)]
pub struct RequestMetrics {
    pub request_information: crate::proxy::ValidRequestInformation,
    pub status_code: http::StatusCode,
    pub req_time: f64,
    pub modules: Vec<module_metric::ModuleMetric>,
}

#[derive(Debug)]
pub struct Metrics;

#[derive(Debug, Clone)]
pub struct MetricsWriter {
    metrics: ::std::sync::Arc<Metrics>,
}

impl RequestMetrics {
    pub fn new(
        request_information: crate::proxy::ValidRequestInformation,
        modules: Vec<module_metric::ModuleMetric>,
        status_code: http::StatusCode,
        req_time: f64,
    ) -> Self {
        Self {
            request_information,
            status_code,
            req_time,
            modules,
        }
    }

    /// Computes a weighted score based on HTTP status, request duration, and module execution times.
    ///
    /// The algorithm weights errors (5xx: 5pts, 4xx: 10pts), response time (req_time * 10),
    /// and each module's execution time (exec_time * 10), summing them into a single cost metric.
    pub fn calculate_score(&self) -> i64 {
        Self::calculate_score_parts(self.status_code, self.req_time, &self.modules)
    }

    pub fn calculate_score_parts(
        status_code: http::StatusCode,
        req_time: f64,
        modules: &[module_metric::ModuleMetric],
    ) -> i64 {
        let status = status_code.as_u16();

        let status_score = match status {
            500..=599 => 5,
            400..=599 if status != 401 && status != 403 => 10,
            _ => 0,
        };

        let time_score = (req_time * 10.0) as i64;

        let module_score: i64 = modules.iter().map(|m| (m.exec_time * 10.0) as i64).sum();

        status_score + time_score + module_score
    }

    pub fn observe_prometheus(&self) {
        let host_str = self.request_information.host.as_str();
        let path_str = self.request_information.path.as_str();
        let method_str = self.request_information.method.to_string();
        let status_str = self.status_code.to_string();
        let destination_str = format!("{:?}", self.request_information.req_match.destination);
        let outcome = if self.status_code.is_server_error() {
            "server_error"
        } else if self.status_code.is_client_error() {
            "client_error"
        } else if self.status_code.is_redirection() {
            "redirect"
        } else {
            "success"
        };

        prom::HTTP_REQUESTS_TOTAL
            .with_label_values(&[
                &method_str,
                &status_str,
                &destination_str,
                outcome,
                host_str,
                path_str,
            ])
            .inc();

        prom::HTTP_RESPONSE_TIME_SECONDS
            .with_label_values(&[&destination_str, &status_str])
            .observe(self.req_time);

        for module in &self.modules {
            let global = format!("{}", module.global);
            let module_replied = module.module_replied.to_string();
            prom::MODULE_EXEC_TIME
                .with_label_values(&[module.name.as_str(), &global, &module_replied])
                .observe(module.exec_time);
        }
    }
}

impl ::std::fmt::Display for RequestMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} {} {} {} - {} {:.2} ms",
            self.request_information.client_information,
            self.request_information.scheme,
            self.request_information.method,
            self.request_information.host,
            self.request_information.path,
            self.status_code,
            self.req_time * 1000.0f64
        )
    }
}

impl ::std::fmt::Debug for RequestMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} {} {} {} - {} {:.2} ms - {:?}",
            self.request_information.client_information,
            self.request_information.scheme,
            self.request_information.method,
            self.request_information.host,
            self.request_information.path,
            self.status_code,
            self.req_time * 1000.0f64,
            self.modules,
        )
    }
}

impl Metrics {
    fn new() -> Self {
        Self
    }

    /// Creates a MetricsWriter backed by the core metrics subsystem.
    pub fn create() -> MetricsWriter {
        MetricsWriter {
            metrics: ::std::sync::Arc::new(Metrics::new()),
        }
    }

    /// Logs a request and records Prometheus metrics.
    pub async fn log_request(&self, http_request: RequestMetrics) {
        tracing::info!("{}", http_request);
        if tracing::enabled!(tracing::Level::DEBUG) {
            tracing::debug!("{:?}", http_request.clone());
        }

        http_request.observe_prometheus();
    }
}

impl MetricsWriter {
    pub async fn log_request(&self, http_request: RequestMetrics) {
        self.metrics.log_request(http_request).await;
    }
}
