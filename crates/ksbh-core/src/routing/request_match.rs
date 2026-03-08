#[derive(Debug, Clone)]
pub struct RequestMatchModule {
    pub name: ::std::sync::Arc<ksbh_types::KsbhStr>,
    pub mod_spec: ::std::sync::Arc<crate::modules::ModuleConfigurationSpec>,
    pub config_kv_slice: ::std::sync::Arc<Vec<crate::modules::abi::ModuleKvSlice>>,
}

#[derive(Debug, Clone)]
pub struct RequestMatch {
    pub backend: super::ServiceBackendType,
    pub modules: Vec<RequestMatchModule>,
}
