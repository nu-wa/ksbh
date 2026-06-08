pub mod active_call;
pub mod error;
pub mod host_functions;
pub mod module_buffer;
pub mod module_host;
pub mod module_instance;

pub use module_buffer::ModuleKvSlice;

#[derive(Debug, Clone)]
pub struct ModuleCallInput<'a> {
    pub stage: ksbh_modules_abi::prelude::KSBHModuleStage,
    pub module_name: &'a str,
    pub module_type: crate::modules::ModuleConfigurationType,
    pub config: &'a crate::modules::ModuleConfigurationValues,
    pub observed: &'a crate::proxy::ObservedRequest,
    pub headers: &'a http::HeaderMap,
    pub body: Option<&'a bytes::Bytes>,
    pub internal_path: &'a str,
    pub needs_session_cookie: bool,
}

#[derive(Debug)]
pub enum ModuleCallOutcome {
    Pass,
    Stop(http::Response<bytes::Bytes>),
    Error(String),
}
