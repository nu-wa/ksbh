/// Handle for reporting metrics to the host.
///
/// Used for score-based rate limiting: modules can reward good behavior
/// (e.g., completing a challenge) with `good_boy()` and query the current
/// score with `get_score()`.
pub struct MetricsHandle {
    good_boy: ksbh_core::modules::abi::MetricsGoodBoyFn,
    get_score: ksbh_core::modules::abi::MetricsGetScoreFn,
}

impl MetricsHandle {
    /// Creates a MetricsHandle from FFI function pointers.
    pub fn from_ffi(
        good_boy: ksbh_core::modules::abi::MetricsGoodBoyFn,
        get_score: ksbh_core::modules::abi::MetricsGetScoreFn,
    ) -> Self {
        Self {
            good_boy,
            get_score,
        }
    }

    /// Rewards good behavior by reducing the client's score by 50.
    ///
    /// Used when a client completes a challenge or demonstrates good behavior.
    /// Returns `true` if the score was updated successfully.
    pub fn good_boy(&self, metrics_key: &[u8]) -> bool {
        unsafe { (self.good_boy)(metrics_key.as_ptr(), metrics_key.len()) }
    }

    /// Gets the current score for a client identified by the metrics key.
    ///
    /// The score is used by the rate-limiting module to make blocking decisions.
    /// Higher scores indicate worse behavior/more requests.
    pub fn get_score(&self, metrics_key: &[u8]) -> u64 {
        unsafe { (self.get_score)(metrics_key.as_ptr(), metrics_key.len()) }
    }
}
