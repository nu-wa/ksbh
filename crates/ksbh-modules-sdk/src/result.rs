pub enum ModuleResult {
    Pass,
    Stop(Option<http::Response<bytes::Bytes>>),
    Error(Option<http::Response<bytes::Bytes>>),
}
