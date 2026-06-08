use std::sync::{
    LazyLock,
    atomic::{AtomicU64, Ordering},
};

const WINDOW_SECONDS: usize = 60;
const WINDOW_SECONDS_U64: u64 = WINDOW_SECONDS as u64;
const RESETTING_BUCKET: u64 = u64::MAX;

#[derive(Debug)]
struct SignalBucket {
    second: AtomicU64,
    requests: AtomicU64,
    errors: AtomicU64,
}

impl SignalBucket {
    fn new() -> Self {
        Self {
            second: AtomicU64::new(0),
            requests: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }
}

#[derive(Debug)]
pub struct RuntimeSignals {
    in_flight_requests: AtomicU64,
    buckets: [SignalBucket; WINDOW_SECONDS],
}

impl RuntimeSignals {
    pub fn new() -> Self {
        Self {
            in_flight_requests: AtomicU64::new(0),
            buckets: std::array::from_fn(|_| SignalBucket::new()),
        }
    }

    pub fn request_started(&self) {
        self.in_flight_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn request_finished(&self) {
        let _ =
            self.in_flight_requests
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_sub(1))
                });
    }

    pub fn observe_completion(&self, status: http::StatusCode) {
        let now = current_unix_seconds();
        let bucket = self.bucket_for_write(now);
        bucket.requests.fetch_add(1, Ordering::Relaxed);
        if status.is_client_error() || status.is_server_error() {
            bucket.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn get(&self, kind: ksbh_modules_abi::functions::KSBHHostSignalKind) -> u64 {
        match kind {
            ksbh_modules_abi::functions::KSBHHostSignalKind::InFlightRequests => {
                self.in_flight_requests.load(Ordering::Relaxed)
            }
            ksbh_modules_abi::functions::KSBHHostSignalKind::RecentRequestsPerMinute => {
                self.recent_requests_per_minute()
            }
            ksbh_modules_abi::functions::KSBHHostSignalKind::RecentErrorRateBps => {
                self.recent_error_rate_bps()
            }
            ksbh_modules_abi::functions::KSBHHostSignalKind::GlobalPressure => {
                self.global_pressure()
            }
        }
    }

    fn recent_requests_per_minute(&self) -> u64 {
        let now = current_unix_seconds();
        self.snapshot(now).0
    }

    fn recent_error_rate_bps(&self) -> u64 {
        let now = current_unix_seconds();
        let (requests, errors) = self.snapshot(now);
        if requests == 0 {
            return 0;
        }

        errors.saturating_mul(10_000) / requests
    }

    fn global_pressure(&self) -> u64 {
        let in_flight = self.in_flight_requests.load(Ordering::Relaxed);
        let now = current_unix_seconds();
        let (recent_requests_per_minute, recent_errors) = self.snapshot(now);
        let recent_error_rate_bps = if recent_requests_per_minute == 0 {
            0
        } else {
            recent_errors.saturating_mul(10_000) / recent_requests_per_minute
        };

        in_flight
            .saturating_mul(10_000)
            .saturating_add(recent_requests_per_minute)
            .saturating_add(recent_error_rate_bps)
    }

    fn bucket_for_write(&self, now: u64) -> &SignalBucket {
        let index = (now % WINDOW_SECONDS_U64) as usize;
        let bucket = &self.buckets[index];

        loop {
            let current_second = bucket.second.load(Ordering::Relaxed);
            if current_second == now {
                return bucket;
            }
            if current_second == RESETTING_BUCKET {
                std::hint::spin_loop();
                continue;
            }

            if bucket
                .second
                .compare_exchange(
                    current_second,
                    RESETTING_BUCKET,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                bucket.requests.store(0, Ordering::Relaxed);
                bucket.errors.store(0, Ordering::Relaxed);
                bucket.second.store(now, Ordering::Release);
                return bucket;
            }
        }
    }

    fn snapshot(&self, now: u64) -> (u64, u64) {
        let mut requests = 0_u64;
        let mut errors = 0_u64;
        let lower_bound = now.saturating_sub(WINDOW_SECONDS_U64 - 1);

        for bucket in &self.buckets {
            let second = bucket.second.load(Ordering::Relaxed);
            if second < lower_bound || second > now {
                continue;
            }

            requests = requests.saturating_add(bucket.requests.load(Ordering::Relaxed));
            errors = errors.saturating_add(bucket.errors.load(Ordering::Relaxed));
        }

        (requests, errors)
    }

    #[cfg(test)]
    fn observe_completion_at(&self, status: http::StatusCode, now: u64) {
        let bucket = self.bucket_for_write(now);
        bucket.requests.fetch_add(1, Ordering::Relaxed);
        if status.is_client_error() || status.is_server_error() {
            bucket.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[cfg(test)]
    fn snapshot_at(&self, now: u64) -> (u64, u64) {
        self.snapshot(now)
    }
}

pub static RUNTIME_SIGNALS: LazyLock<RuntimeSignals> = LazyLock::new(RuntimeSignals::new);

fn current_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_in_flight_requests_and_completion_counts() {
        let signals = RuntimeSignals::new();

        signals.request_started();
        signals.observe_completion_at(http::StatusCode::OK, 1_000);

        assert_eq!(
            signals.get(ksbh_modules_abi::functions::KSBHHostSignalKind::InFlightRequests),
            1
        );
        assert_eq!(
            signals.snapshot_at(1_000),
            (1, 0),
            "request counts should include the current second"
        );

        signals.request_finished();

        assert_eq!(
            signals.get(ksbh_modules_abi::functions::KSBHHostSignalKind::InFlightRequests),
            0
        );
    }

    #[test]
    fn computes_recent_error_rate_over_the_window() {
        let signals = RuntimeSignals::new();

        signals.observe_completion_at(http::StatusCode::OK, 2_000);
        signals.observe_completion_at(http::StatusCode::NOT_FOUND, 2_001);
        signals.observe_completion_at(http::StatusCode::INTERNAL_SERVER_ERROR, 2_001);

        let (requests, errors) = signals.snapshot_at(2_001);
        assert_eq!(requests, 3);
        assert_eq!(errors, 2);
        assert_eq!(errors.saturating_mul(10_000) / requests, 6666);

        signals.observe_completion_at(http::StatusCode::OK, 2_061);

        assert_eq!(
            signals.snapshot_at(2_061),
            (1, 0),
            "older buckets should fall out of the fixed window"
        );
    }
}
