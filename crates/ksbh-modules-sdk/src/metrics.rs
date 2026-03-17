pub struct MetricsHandle {
    good_boy: ksbh_core::modules::abi::MetricsGoodBoyFn,
    get_score: ksbh_core::modules::abi::MetricsGetScoreFn,
}

impl MetricsHandle {
    pub fn from_ffi(
        good_boy: ksbh_core::modules::abi::MetricsGoodBoyFn,
        get_score: ksbh_core::modules::abi::MetricsGetScoreFn,
    ) -> Self {
        Self {
            good_boy,
            get_score,
        }
    }

    pub fn good_boy(&self, metrics_key: &[u8]) -> bool {
        unsafe { (self.good_boy)(metrics_key.as_ptr(), metrics_key.len()) }
    }

    pub fn get_score(&self, metrics_key: &[u8]) -> u64 {
        unsafe { (self.get_score)(metrics_key.as_ptr(), metrics_key.len()) }
    }
}
