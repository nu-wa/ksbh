use crate::functions::{
    HostFnFreeBytes, HostFnLog, HostFnReputationGetScore, HostFnReputationGoodBoy,
    HostFnSessionGet, HostFnSessionSet, HostFnSessionSharedGet, HostFnSessionSharedSet,
    HostFnSignalGet,
};

#[repr(C)]
pub struct KSBHBytes {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
pub struct SessionID {
    pub inner: [u8; 16],
}

#[repr(C)]
pub struct KSBHString {
    pub inner: KSBHBytes,
}

#[repr(C)]
pub struct KSBHKVString {
    pub key: KSBHString,
    pub value: KSBHString,
}

#[repr(C)]
pub struct KSBHKVBytes {
    pub key: KSBHBytes,
    pub value: KSBHBytes,
}

#[repr(C)]
pub struct KSBHSliceBytes {
    pub ptr: *const KSBHBytes,
    pub len: usize,
}

#[repr(C)]
pub struct KSBHSliceStrings {
    pub ptr: *const KSBHString,
    pub len: usize,
}

#[repr(C)]
pub struct KSBHSliceKVStrings {
    pub ptr: *const KSBHKVString,
    pub len: usize,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KSBHModuleKind {
    OIDC = 0,
    POW = 1,
    HttpToHttps = 2,
    RateLimit = 3,
    Robots = 4,
    Custom = 100,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KSBHModuleStage {
    // Pingora Early Request Filter
    BeforeRouting = 0,
    // Pingora Request Filter if host match
    Request = 1,
    // After request
    Logging = 3,
}

#[repr(C)]
pub struct KSBHModuleStages {
    pub ptr: *const KSBHModuleStage,
    pub len: usize,
}

#[repr(C)]
pub struct ModuleInfo {
    pub kind: KSBHModuleKind,
    // Only used for KSBHModuleKind::Custom
    pub name: KSBHString,
    pub registered_stages: KSBHModuleStages,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KSBHHostFnReturn {
    Success = 0,
    HostFailure = 1,
    BadArgument = 2,
    NotFound = 3,
}

#[repr(C)]
pub struct KSBHHostCtxHandle {
    pub inner: u64,
}

// Sent from Host to module and is only valid for a single handle_request_fn.
// A different `KSBHHostCtxHandle` will be given to the module per `KSBHModuleStage` call, meaning
// per handle_request_fn call. A module that registers 2 stages will be called twice using a
// different `KSBHHostCtxHandle` for each call to `handle_request_fn`
#[repr(C)]
pub struct ModuleContext {
    pub host_ctx: KSBHHostCtxHandle,
    pub config: KSBHSliceKVStrings,
    pub headers: KSBHSliceKVStrings,
    pub request_info: *const crate::request_info::RequestInfo,
    pub body: KSBHBytes,
    pub cookie_header: KSBHString,
    pub reputation_key: KSBHBytes,
    pub internal_path: KSBHString,
    pub session_id: SessionID,
    pub h_log_fn: HostFnLog,
    pub h_free_bytes: HostFnFreeBytes,
    pub h_reputation_good_boy_fn: HostFnReputationGoodBoy,
    pub h_reputation_get_score_fn: HostFnReputationGetScore,
    pub h_signal_get_fn: HostFnSignalGet,
    pub h_session_get_fn: HostFnSessionGet,
    pub h_session_set_fn: HostFnSessionSet,
    pub h_session_shared_get_fn: HostFnSessionSharedGet,
    pub h_session_shared_set_fn: HostFnSessionSharedSet,
}

// Owned by a module, must be freed using `crate::functions::ModuleFnFreeModuleResponse` after host
// decides its done with parsing it.
#[repr(C)]
pub struct ModuleResponse {
    // Generic buffer to be used by host depending on decision,
    // if KSBHModuleDecision::Error, module can add a detail for logging or something
    pub body: KSBHBytes,
    pub decision: KSBHModuleDecision,
    // Modified headers
    pub headers: KSBHSliceKVStrings,
    pub status_code: u16,
}

#[repr(u16)]
pub enum KSBHModuleDecision {
    Pass = 0,
    Stop = 1,
    // Module failed in its process and cannot keep going
    Error = 2,
}

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}
