pub enum ModuleResult {
    Pass,
    Stop(http::Response<bytes::Bytes>),
}
