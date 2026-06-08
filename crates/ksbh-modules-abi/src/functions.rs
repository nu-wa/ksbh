use crate::types::*;

pub type HostFnLog = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    level: LogLevel,
    message: KSBHBytes,
) -> KSBHHostFnReturn;

pub type HostFnFreeBytes = unsafe extern "C" fn(ctx_handle: KSBHHostCtxHandle, data: KSBHBytes);

/// Populates `mut u64` with the result
pub type HostFnReputationGetScore = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    *mut u64,
) -> KSBHHostFnReturn;

pub type HostFnReputationGoodBoy =
    unsafe extern "C" fn(ctx_handle: KSBHHostCtxHandle) -> KSBHHostFnReturn;

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KSBHHostSignalKind {
    GlobalPressure = 0,
    InFlightRequests = 1,
    RecentRequestsPerMinute = 2,
    RecentErrorRateBps = 3,
}

pub type HostFnSignalGet = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    kind: KSBHHostSignalKind,
    out: *mut u64,
) -> KSBHHostFnReturn;

pub type HostFnSessionGet = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    out_ptr: *mut KSBHBytes,
) -> KSBHHostFnReturn;

pub type HostFnSessionSet = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    value: KSBHBytes,
    ttl: u64,
) -> KSBHHostFnReturn;

pub type HostFnSessionSharedGet = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    out_ptr: *mut KSBHBytes,
) -> KSBHHostFnReturn;

pub type HostFnSessionSharedSet = unsafe extern "C" fn(
    ctx_handle: KSBHHostCtxHandle,
    key: KSBHBytes,
    value: KSBHBytes,
    ttl: u64,
) -> KSBHHostFnReturn;

pub type ModuleFnHandleRequest = unsafe extern "C" fn(
    stage: KSBHModuleStage,
    ctx: *const ModuleContext,
) -> *const ModuleResponse;

pub type ModuleFnFreeModuleResponse =
    unsafe extern "C" fn(ctx: *const ModuleContext, *mut ModuleResponse);

pub type ModuleFnGetDescriptor =
    unsafe extern "C" fn() -> crate::module_descriptor::ModuleDescriptor;
