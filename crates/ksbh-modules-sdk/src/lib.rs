pub mod context;
pub mod error;
pub mod ffi;
pub mod logger;
pub mod metrics;
pub mod result;
pub mod session;
pub mod types;

pub use context::{RequestContext, RequestInfo};
pub use error::ModuleError;
pub use ffi::OwnedResponse;
pub use metrics::MetricsHandle;
pub use result::ModuleResult;

/// Free a response allocated by the module.
///
/// This function should be called to free responses returned by `request_filter`
/// when the response is not null.
///
/// Currently this is a no-op as the SDK manages memory internally.
///
/// # Safety
///
/// The pointers must have been obtained from a call to the module's
/// `request_filter` function. Calling this with null pointers or pointers
/// from another source results in undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_response(
    _headers_ptr: *const ksbh_core::modules::abi::ModuleKvSlice,
    _headers_len: usize,
    _body_ptr: *const u8,
    _body_len: usize,
) {
    // Headers are kept alive by a static in the SDK
    // Body is owned by ModuleResponse which is dropped when the host
    // finishes processing
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

            // Catch panics from module code to prevent crashes
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                $func($crate::ffi::convert_context(unsafe { &*ctx }))
            }));

            match result {
                Ok(Ok($crate::ModuleResult::Pass)) => std::ptr::null(),
                Ok(Ok($crate::ModuleResult::Stop(resp))) => $crate::ffi::alloc_response(resp),
                Ok(Err(e)) => {
                    // Module returned an error - return 500 with error message
                    // The module already logged/handled the error appropriately
                    let message = e.to_string();
                    tracing::warn!("Module returned error: {}", message);
                    let resp = http::Response::builder()
                        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(bytes::Bytes::from(message))
                        .unwrap();
                    $crate::ffi::alloc_response(resp)
                }
                Err(_) => {
                    // Module panicked - return 500
                    tracing::error!("Module panicked");
                    let resp = http::Response::builder()
                        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(bytes::Bytes::from("Module panic"))
                        .unwrap();
                    $crate::ffi::alloc_response(resp)
                }
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
