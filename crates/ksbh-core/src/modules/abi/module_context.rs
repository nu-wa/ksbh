#[repr(C)]
pub struct ModuleContext<'a> {
    pub config: &'a [super::ModuleKvSlice],
    pub headers: &'a [super::ModuleKvSlice],
    pub req_info: &'a super::RequestInfo,
    pub body: &'a [u8],
    pub log_fn: super::log::LogFn,
    pub session_id: [u8; 16],
    pub session_get_fn: super::SessionGetFn,
    pub session_set_fn: super::SessionSetFn,
    pub session_set_with_ttl_fn: super::SessionSetWithTtlFn,
    pub session_free_fn: super::SessionFreeFn,
    pub mod_name: &'a str,
    pub cookie_header: super::ModuleBuffer,
    pub metrics_key: super::ModuleBuffer,
    pub metrics_good_boy_fn: super::MetricsGoodBoyFn,
    pub metrics_get_score_fn: super::MetricsGetScoreFn,
    pub internal_path: super::ModuleBuffer,
}
