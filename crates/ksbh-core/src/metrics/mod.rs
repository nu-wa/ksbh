//! Everything related to metrics

pub mod module_metric;
pub mod prom;

pub use prometheus;

#[derive(Debug, Clone)]
pub enum RequestStage {
    EarlyFilter,
    RequestFilter,
}

// Request data for metrics
#[derive(Clone)]
pub struct RequestMetrics {
    client_information: crate::proxy::PartialClientInformation,
    http_request_info: ksbh_types::prelude::HttpRequest,
    backend_type: crate::routing::ServiceBackendType,
    cookie_set: bool,
    status_code: http::StatusCode,
    req_time: f64,
    modules: Vec<module_metric::ModuleMetric>,
}

/// Simple struct to represent a request information, for now it's very basic.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Hits {
    pub logged_at: chrono::NaiveDateTime,
    pub good: u16,
    pub bad: u16,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    http_requests: scc::HashMap<
        crate::proxy::PartialClientInformation,
        ::std::collections::VecDeque<RequestMetrics>,
    >,
    http_hits:
        crate::storage::redis_hashmap::RedisHashMap<crate::proxy::PartialClientInformation, Hits>,
}

#[derive(Debug, Clone)]
pub struct MetricsWriter {
    metrics: ::std::sync::Arc<Metrics>,
}

#[derive(Debug, Clone)]
pub struct MetricsReader {
    metrics: ::std::sync::Arc<Metrics>,
}

impl RequestMetrics {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client_information: crate::proxy::PartialClientInformation,
        http_request_info: ksbh_types::prelude::HttpRequest,
        backend_type: crate::routing::ServiceBackendType,
        cookie_set: bool,
        status_code: http::StatusCode,
        modules: Vec<module_metric::ModuleMetric>,
        req_time: f64,
    ) -> Self {
        Self {
            client_information,
            http_request_info,
            backend_type,
            cookie_set,
            status_code,
            req_time,
            modules,
        }
    }
}

impl ::std::fmt::Display for RequestStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::RequestFilter => "RequestFilter",
                Self::EarlyFilter => "EarlyRequestFilter",
            }
        )
    }
}

impl ::std::fmt::Display for RequestMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} {} '{}' {:.2} ms",
            self.client_information,
            self.http_request_info.method,
            self.status_code,
            self.http_request_info.uri,
            self.req_time * 1000.0f64
        )
    }
}

impl ::std::fmt::Debug for RequestMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} {} '{}' {:.2} ms - {} - {:?} {:?}",
            self.client_information,
            self.http_request_info.method,
            self.status_code,
            self.http_request_info.uri,
            self.req_time * 1000.0f64,
            self.cookie_set,
            self.backend_type,
            self.modules,
        )
    }
}

impl Metrics {
    fn new(storage: Option<::std::sync::Arc<crate::Storage>>) -> Self {
        Self {
            http_requests: scc::HashMap::new(),
            http_hits: crate::RedisHashMap::new(
                Some(::std::time::Duration::from_mins(15)),
                Some(::std::time::Duration::from_hours(24)),
                storage,
            ),
        }
    }

    pub fn create(
        storage: Option<::std::sync::Arc<crate::Storage>>,
    ) -> (MetricsWriter, MetricsReader) {
        let metrics = ::std::sync::Arc::new(Metrics::new(storage));

        (
            MetricsWriter {
                metrics: metrics.clone(),
            },
            MetricsReader { metrics },
        )
    }

    async fn add_http_request(&self, http_request: RequestMetrics) {
        if tracing::enabled!(tracing::Level::DEBUG) {
            tracing::debug!("{:?}", http_request);
        } else {
            tracing::info!("{}", http_request);
        }
        let key = http_request.client_information.clone();

        let mut previous = match self.http_requests.get_sync(&key) {
            Some(previous) => previous.to_owned(),
            None => ::std::collections::VecDeque::new(),
        };
        let mut previous_hits = match self.http_hits.get_cold(key.clone()).await {
            Some(prev) => prev.get().inner(),
            None => Hits {
                good: 0,
                bad: 0,
                logged_at: chrono::Utc::now().naive_utc(),
            },
        };

        let status_code_u16 = http_request.status_code.as_u16();
        let mut outcome = "good";

        if status_code_u16 >= 400 && status_code_u16 != 401 && status_code_u16 != 403 {
            previous_hits.bad += 1;
            outcome = "bad";
        } else {
            previous_hits.good += 1;
        }
        let status_str = http_request.status_code.to_string();
        let method_str = http_request.http_request_info.method.to_string();
        let backend_str = format!("{:?}", http_request.backend_type);

        previous.push_back(http_request.clone());

        self.http_requests.upsert_sync(key.clone(), previous);
        self.http_hits.upsert(key, previous_hits).await;

        prom::HTTP_REQUESTS_TOTAL
            .with_label_values(&[&method_str, &status_str, &backend_str, outcome])
            .inc();

        prom::HTTP_RESPONSE_TIME_SECONDS
            .with_label_values(&[&backend_str, &status_str])
            .observe(http_request.req_time);

        for module in &http_request.modules {
            let stage = format!("{}", module.stage);
            let global = format!("{}", module.global);
            let decision = match &module.decision {
                Some(dec) => dec.to_string(),
                None => "None".to_string(),
            };
            prom::MODULE_EXEC_TIME
                .with_label_values(&[module.name.as_str(), &stage, &global, &decision])
                .observe(module.exec_time);
        }
    }

    async fn good_boy(&self, key: crate::proxy::PartialClientInformation) {
        let mut previous_or_new = self.get_hits_or_new(key.clone()).await;

        // Let's not wipe everything, we keep the previous amount of bad requests + n
        // TODO: change the hardcoded 5 maybe ?
        previous_or_new.good += previous_or_new.bad + 5;

        self.http_hits.upsert(key, previous_or_new).await;
    }

    async fn clean_http_requests(
        &self,
        interval: ::std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        self.http_hits.watch(interval).await
    }

    fn get_http_requests_full(
        &self,
    ) -> &scc::HashMap<
        crate::proxy::PartialClientInformation,
        ::std::collections::VecDeque<RequestMetrics>,
    > {
        &self.http_requests
    }

    fn get_http_requests(
        &self,
        key: &crate::proxy::PartialClientInformation,
    ) -> Option<::std::collections::VecDeque<RequestMetrics>> {
        self.http_requests.read_sync(key, |_, v| v.clone())
    }

    async fn get_http_hits(&self, key: &crate::proxy::PartialClientInformation) -> Option<Hits> {
        self.http_hits
            .get_cold(key.clone())
            .await
            .map(|v| v.inner())
    }

    async fn get_hits_or_new(&self, key: crate::proxy::PartialClientInformation) -> Hits {
        match self.http_hits.get_cold(key).await {
            Some(prev) => prev.get().inner(),
            None => Hits {
                good: 0,
                bad: 0,
                logged_at: chrono::Local::now().naive_local(),
            },
        }
    }
}

impl MetricsReader {
    pub fn get_http_requests_full(
        &self,
    ) -> &scc::HashMap<
        crate::proxy::PartialClientInformation,
        ::std::collections::VecDeque<RequestMetrics>,
    > {
        self.metrics.get_http_requests_full()
    }

    pub fn get_http_requests(
        &self,
        key: &crate::proxy::PartialClientInformation,
    ) -> Option<::std::collections::VecDeque<RequestMetrics>> {
        self.metrics.get_http_requests(key)
    }

    pub async fn get_http_hits(
        &self,
        key: &crate::proxy::PartialClientInformation,
    ) -> Option<Hits> {
        self.metrics.get_http_hits(key).await
    }
}

impl MetricsWriter {
    pub fn http_hits(
        &self,
    ) -> &crate::storage::redis_hashmap::RedisHashMap<crate::proxy::PartialClientInformation, Hits>
    {
        &self.metrics.http_hits
    }

    pub async fn add_http_request(&self, http_request: RequestMetrics) {
        self.metrics.add_http_request(http_request).await;
    }

    pub async fn clean_http_requests(
        &self,
        interval: ::std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        self.metrics.clean_http_requests(interval).await
    }

    pub fn get_http_requests_full(
        &self,
    ) -> &scc::HashMap<
        crate::proxy::PartialClientInformation,
        ::std::collections::VecDeque<RequestMetrics>,
    > {
        self.metrics.get_http_requests_full()
    }

    pub fn get_http_requests(
        &self,
        key: &crate::proxy::PartialClientInformation,
    ) -> Option<::std::collections::VecDeque<RequestMetrics>> {
        self.metrics.get_http_requests(key)
    }

    pub async fn get_http_hits(
        &self,
        key: &crate::proxy::PartialClientInformation,
    ) -> Option<Hits> {
        self.metrics.get_http_hits(key).await
    }

    pub async fn good_boy(&self, key: crate::proxy::PartialClientInformation) {
        self.metrics.good_boy(key).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_stage_request_filter() {
        let stage = RequestStage::RequestFilter;
        assert_eq!(format!("{}", stage), "RequestFilter");
    }

    #[test]
    fn test_request_stage_early_filter() {
        let stage = RequestStage::EarlyFilter;
        assert_eq!(format!("{}", stage), "EarlyRequestFilter");
    }

    #[test]
    fn test_request_stage_debug() {
        let stage = RequestStage::RequestFilter;
        assert_eq!(format!("{:?}", stage), "RequestFilter");
    }

    #[test]
    fn test_hits_default() {
        let hits = Hits {
            logged_at: chrono::Utc::now().naive_utc(),
            good: 0,
            bad: 0,
        };
        assert_eq!(hits.good, 0);
        assert_eq!(hits.bad, 0);
    }

    #[test]
    fn test_hits_with_values() {
        let hits = Hits {
            logged_at: chrono::Utc::now().naive_utc(),
            good: 10,
            bad: 5,
        };
        assert_eq!(hits.good, 10);
        assert_eq!(hits.bad, 5);
    }

    #[test]
    fn test_hits_serialize() {
        let hits = Hits {
            logged_at: chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc(),
            good: 10,
            bad: 5,
        };

        let serialized = serde_json::to_string(&hits).unwrap();
        assert!(serialized.contains("10"));
    }

    #[test]
    fn test_hits_deserialize() {
        let json = r#"{"logged_at":"1970-01-01T00:00:00","good":10,"bad":5}"#;
        let hits: Hits = serde_json::from_str(json).unwrap();
        assert_eq!(hits.good, 10);
        assert_eq!(hits.bad, 5);
    }

    #[test]
    fn test_request_metrics_display() {
        let client_info = crate::proxy::PartialClientInformation {
            ip: "127.0.0.1".parse().unwrap(),
            user_agent: Some(ksbh_types::KsbhStr::new("test")),
        };

        let http_info =
            ksbh_types::prelude::HttpRequest::t_create("localhost", Some(b"/test"), Some("GET"));

        let metrics = RequestMetrics::new(
            client_info,
            http_info,
            crate::routing::ServiceBackendType::None,
            false,
            http::StatusCode::OK,
            vec![],
            0.1,
        );

        let display = format!("{}", metrics);
        assert!(display.contains("127.0.0.1"));
        assert!(display.contains("GET"));
    }
}
