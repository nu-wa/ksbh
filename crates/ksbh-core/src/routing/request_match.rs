#[derive(Debug, Clone)]
pub struct RequestMatchModule {
    pub name: ::std::sync::Arc<ksbh_types::KsbhStr>,
    pub mod_spec: ::std::sync::Arc<crate::modules::ModuleConfigurationSpec>,
    pub config_kv_slice: ::std::sync::Arc<Vec<crate::modules::abi::ModuleKvSlice>>,
}

pub type RequestMatchModules = ::std::sync::Arc<[RequestMatchModule]>;

#[derive(Debug, Clone)]
pub struct RequestMatch {
    pub backend: super::ServiceBackendType,
    pub modules: RequestMatchModules,
    pub https: bool,
}
