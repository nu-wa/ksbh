pub mod context;
pub mod ffi;
pub mod logger;
pub mod metrics;
pub mod result;
pub mod session;
pub mod types;

pub use context::{RequestContext, RequestInfo};
pub use ffi::OwnedResponse;
pub use metrics::MetricsHandle;
pub use result::ModuleResult;

/// Free a response allocated by the module.
///
/// This function should be called to free responses returned by `request_filter`
/// when the response is not null.
///
/// # Safety
///
/// The `resp` pointer must have been obtained from a call to the module's
/// `request_filter` function. Calling this with a null pointer or a pointer
/// from another source results in undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_response(resp: *const ksbh_core::modules::abi::ModuleResponse) {
    if !resp.is_null() {
        // The pointer actually points to an OwnedResponse wrapper
        // We need to cast it back and drop it to free the owned allocations
        let _owned = unsafe { Box::from_raw(resp as *mut crate::ffi::OwnedResponse) };
    }
}

#[macro_export]
macro_rules! register_module {
    ($func:path, $type:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn get_module_type() -> ksbh_core::modules::abi::ModuleType {
            $type.to_ffi()
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn request_filter(
            ctx: *const ksbh_core::modules::abi::ModuleContext<'_>,
        ) -> *const ksbh_core::modules::abi::ModuleResponse {
            if ctx.is_null() {
                return std::ptr::null();
            }
            let ctx = $crate::ffi::convert_context(unsafe { &*ctx });
            match $func(ctx) {
                $crate::ModuleResult::Pass => std::ptr::null(),
                $crate::ModuleResult::Stop(resp) => $crate::ffi::alloc_response(resp),
            }
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(0, ::std::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(1, ::std::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(2, ::std::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(3, ::std::format_args!($($arg)*))
    };
}
