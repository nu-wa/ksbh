pub struct MetricsHandle {
    increment_good: ksbh_core::modules::abi::MetricsIncrementGoodFn,
    get_hits: ksbh_core::modules::abi::MetricsGetHitsFn,
}

impl MetricsHandle {
    pub fn from_ffi(
        increment_good: ksbh_core::modules::abi::MetricsIncrementGoodFn,
        get_hits: ksbh_core::modules::abi::MetricsGetHitsFn,
    ) -> Self {
        Self {
            increment_good,
            get_hits,
        }
    }

    pub fn increment_good(&self, client_ip: &str, user_agent: Option<&str>) -> bool {
        let ua_ptr = user_agent.map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
        let ua_len = user_agent.map(|s| s.len()).unwrap_or(0);

        unsafe { (self.increment_good)(client_ip.as_ptr(), client_ip.len(), ua_ptr, ua_len) }
    }

    pub fn get_hits(&self, client_ip: &str, user_agent: Option<&str>) -> Option<(u32, u32)> {
        let mut out_good: u32 = 0;
        let mut out_bad: u32 = 0;

        let ua_ptr = user_agent.map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
        let ua_len = user_agent.map(|s| s.len()).unwrap_or(0);

        let success = unsafe {
            (self.get_hits)(
                client_ip.as_ptr(),
                client_ip.len(),
                ua_ptr,
                ua_len,
                &mut out_good,
                &mut out_bad,
            )
        };

        if success {
            Some((out_good, out_bad))
        } else {
            None
        }
    }
}
