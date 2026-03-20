//! Everything related to metrics

pub mod module_metric;
pub mod prom;

pub use prometheus;

#[derive(Debug)]
pub struct AtomicU64Wrapper {
    value: ::std::sync::atomic::AtomicU64,
}

impl Clone for AtomicU64Wrapper {
    fn clone(&self) -> Self {
        Self {
            value: ::std::sync::atomic::AtomicU64::new(self.load()),
        }
    }
}

impl AtomicU64Wrapper {
    pub fn new(val: u64) -> Self {
        Self {
            value: ::std::sync::atomic::AtomicU64::new(val),
        }
    }

    pub fn load(&self) -> u64 {
        self.value.load(::std::sync::atomic::Ordering::Relaxed)
    }

    pub fn fetch_add(&self, delta: u64) -> u64 {
        self.value
            .fetch_add(delta, ::std::sync::atomic::Ordering::Relaxed)
    }

    pub fn fetch_sub(&self, delta: u64) -> u64 {
        self.value
            .fetch_sub(delta, ::std::sync::atomic::Ordering::Relaxed)
    }
}

impl serde::Serialize for AtomicU64Wrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.load())
    }
}

impl<'de> serde::Deserialize<'de> for AtomicU64Wrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::new(u64::deserialize(deserializer)?))
    }
}

// Request data for metrics
#[derive(Clone)]
pub struct RequestMetrics {
    pub request_information: crate::proxy::ValidRequestInformation,
    pub status_code: http::StatusCode,
    pub req_time: f64,
    pub modules: Vec<module_metric::ModuleMetric>,
}

pub struct RequestScore {
    score: ::std::sync::atomic::AtomicI64,
    request_count: ::std::sync::atomic::AtomicU64,
    window_start: ::std::sync::atomic::AtomicU64,
}

impl ::std::fmt::Debug for RequestScore {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "RequestScore {{ score: {}, count: {}, window: {} }}",
            self.score.load(::std::sync::atomic::Ordering::Relaxed),
            self.request_count
                .load(::std::sync::atomic::Ordering::Relaxed),
            self.window_start
                .load(::std::sync::atomic::Ordering::Relaxed)
        )
    }
}

impl Clone for RequestScore {
    fn clone(&self) -> Self {
        Self {
            score: ::std::sync::atomic::AtomicI64::new(
                self.score.load(::std::sync::atomic::Ordering::Relaxed),
            ),
            request_count: ::std::sync::atomic::AtomicU64::new(
                self.request_count
                    .load(::std::sync::atomic::Ordering::Relaxed),
            ),
            window_start: ::std::sync::atomic::AtomicU64::new(
                self.window_start
                    .load(::std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl Default for RequestScore {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestScore {
    pub fn new() -> Self {
        Self {
            score: ::std::sync::atomic::AtomicI64::new(0),
            request_count: ::std::sync::atomic::AtomicU64::new(0),
            window_start: ::std::sync::atomic::AtomicU64::new(
                crate::utils::current_unix_time() as u64
            ),
        }
    }

    fn get_window_duration(request_count: u64) -> ::std::time::Duration {
        let base = ::std::time::Duration::from_secs(10 * 60);
        let scaling = request_count as f64 / 1000.0;
        let scaled = base.as_secs_f64() / scaling.max(1.0);
        ::std::time::Duration::from_secs(scaled.clamp(60.0, 3600.0) as u64)
    }

    pub fn add_score(&self, delta: i64) {
        self.score
            .fetch_add(delta, ::std::sync::atomic::Ordering::Relaxed);
        self.request_count
            .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_score(&self) -> i64 {
        self.score.load(::std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_request_count(&self) -> u64 {
        self.request_count
            .load(::std::sync::atomic::Ordering::Relaxed)
    }

    pub fn should_reset(&self) -> bool {
        let window_start = self
            .window_start
            .load(::std::sync::atomic::Ordering::Relaxed);
        let request_count = self
            .request_count
            .load(::std::sync::atomic::Ordering::Relaxed);
        let window_duration = Self::get_window_duration(request_count);
        let elapsed = crate::utils::current_unix_time() - window_start as i64;
        elapsed > window_duration.as_secs() as i64
    }

    pub fn reset(&self) {
        self.score.store(0, ::std::sync::atomic::Ordering::Relaxed);
        self.request_count
            .store(0, ::std::sync::atomic::Ordering::Relaxed);
        self.window_start.store(
            crate::utils::current_unix_time() as u64,
            ::std::sync::atomic::Ordering::Relaxed,
        );
    }

    pub fn subtract(&self, delta: i64) {
        self.score
            .fetch_sub(delta, ::std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct Metrics {
    pub(crate) scores: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
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

    pub fn calculate_score(&self) -> i64 {
        let status = self.status_code.as_u16();

        let status_score = match status {
            500..=599 => 5,
            400..=599 if status != 401 && status != 403 => 10,
            _ => 1,
        };

        let time_score = (self.req_time * 10.0) as i64;

        let module_score: i64 = self
            .modules
            .iter()
            .map(|m| (m.exec_time * 10.0) as i64)
            .sum();

        status_score + time_score + module_score
    }

    pub fn observe_prometheus(&self) {
        let host_str = self.request_information.host.as_str();
        let path_str = self.request_information.path.as_str();
        let method_str = self.request_information.method.to_string();
        let status_str = self.status_code.to_string();
        let backend_str = format!("{:?}", self.request_information.req_match.backend);
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
                &backend_str,
                outcome,
                host_str,
                path_str,
            ])
            .inc();

        prom::HTTP_RESPONSE_TIME_SECONDS
            .with_label_values(&[&backend_str, &status_str])
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
            "{} - {} {} {} - {} {:.2} ms",
            self.request_information.client_information,
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
            "{} - {} {} {} - {} {:.2} ms - {:?}",
            self.request_information.client_information,
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
    fn new(
        store: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    ) -> Self {
        Self { scores: store }
    }

    pub fn create(
        store: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    ) -> (MetricsWriter, MetricsReader) {
        let metrics = ::std::sync::Arc::new(Metrics::new(store));

        (
            MetricsWriter {
                metrics: metrics.clone(),
            },
            MetricsReader { metrics },
        )
    }

    pub async fn log_request(&self, http_request: RequestMetrics) {
        let total_score = http_request.calculate_score();
        let session_id = http_request.request_information.session_id;
        let key = crate::storage::module_session_key::ModuleSessionKey::user_session(session_id);

        if let Some(score_bytes) = self.scores.get_hot_sync(&key) {
            let score: AtomicU64Wrapper =
                rmp_serde::from_slice(&score_bytes).unwrap_or_else(|_| AtomicU64Wrapper::new(0));
            let new_value = AtomicU64Wrapper::new(score.load().saturating_add(total_score as u64));
            let encoded = rmp_serde::to_vec(&new_value).unwrap_or(score_bytes);
            let _ = self.scores.set_sync(key, encoded);
        } else {
            let new_value = AtomicU64Wrapper::new(total_score as u64);
            let encoded = rmp_serde::to_vec(&new_value).unwrap_or_else(|_| vec![]);
            let _ = self.scores.set_sync(key, encoded);
        }

        tracing::info!("{}", http_request);
        if tracing::enabled!(tracing::Level::DEBUG) {
            tracing::debug!("{:?}", http_request.clone());
        }

        http_request.observe_prometheus();
    }

    async fn good_boy(&self, session_id: uuid::Uuid) {
        let key = crate::storage::module_session_key::ModuleSessionKey::user_session(session_id);
        if let Some(score_bytes) = self.scores.get_hot_sync(&key) {
            let score: AtomicU64Wrapper =
                rmp_serde::from_slice(&score_bytes).unwrap_or_else(|_| AtomicU64Wrapper::new(0));
            let new_value = AtomicU64Wrapper::new(score.load().saturating_sub(50));
            let encoded = rmp_serde::to_vec(&new_value).unwrap_or(score_bytes);
            let _ = self.scores.set_sync(key, encoded);
        }
    }

    async fn cleanup(&self, _interval: ::std::time::Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        })
    }

    async fn get_score(&self, session_id: &uuid::Uuid) -> Option<u64> {
        let key = crate::storage::module_session_key::ModuleSessionKey::user_session(*session_id);
        self.scores.get_hot_sync(&key).and_then(|score_bytes| {
            rmp_serde::from_slice::<AtomicU64Wrapper>(&score_bytes)
                .ok()
                .map(|s| s.load())
        })
    }
}

impl MetricsReader {
    pub async fn get_score(&self, key: &uuid::Uuid) -> Option<u64> {
        self.metrics.get_score(key).await
    }
}

impl MetricsWriter {
    pub async fn log_request(&self, http_request: RequestMetrics) {
        self.metrics.log_request(http_request).await;
    }

    pub async fn cleanup(&self, interval: ::std::time::Duration) -> tokio::task::JoinHandle<()> {
        self.metrics.cleanup(interval).await
    }

    pub async fn good_boy(&self, key: uuid::Uuid) {
        self.metrics.good_boy(key).await;
    }

    pub fn scores(
        &self,
    ) -> ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    > {
        self.metrics.scores.clone()
    }
}
