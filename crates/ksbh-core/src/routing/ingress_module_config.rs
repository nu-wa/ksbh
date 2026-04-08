#[derive(Debug, Clone, Default)]
pub struct IngressModuleConfig {
    pub modules: Vec<::std::sync::Arc<str>>,
    pub excluded_modules: Vec<::std::sync::Arc<str>>,
}
