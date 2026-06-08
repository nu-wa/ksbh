use crate::functions::*;
use crate::types::*;
use crate::version::KsbhAbiVersion;

// Loaded by host to get information about the module
#[repr(C)]
pub struct ModuleDescriptor {
    pub magic: u64,
    pub abi_version: KsbhAbiVersion,
    pub info: ModuleInfo,
    pub handle_request_fn: ModuleFnHandleRequest,
    pub free_module_response_fn: ModuleFnFreeModuleResponse,
}
