//! SDK for building FFI modules for the KSBH reverse proxy.
//!
//! This crate provides a convenient Rust API for building dynamically-loaded modules
//! that interface with KSBH via the FFI ABI defined in `ksbh_core::modules::abi`.
//!
//! # Core Components
//!
//! - [`context::RequestContext`] - Safe wrapper around the raw module context,
//!   providing access to request data, headers, session storage, and metrics
//! - [`result::ModuleResult`] - Return type for module request handlers
//!   (`Pass` to continue, `Stop(Response)` to return immediately)
//! - [`error::ModuleError`] - Error type with convenience constructors for
//!   common HTTP status codes
//! - [`session::SessionHandle`] - Read/write session data with TTL support
//! - [`metrics::MetricsHandle`] - Report metrics to the host (score tracking)
//! - [`logger::Logger`] - Log messages via the host's logging infrastructure
//!
//! # Module Entry Point
//!
//! Modules must implement a `process(ctx: RequestContext) -> Result<ModuleResult, ModuleError>`
//! function and use the [`register_module!`] macro to export the required FFI functions.
//!
//! # Example
//!
//! ```ignore
//! fn handle_request(
//!     mut ctx: ksbh_modules_sdk::RequestContext<'_>,
//! ) -> ksbh_modules_sdk::ModuleResult {
//!     let path = ctx.request().path();
//!     // ... process request
//!     ksbh_modules_sdk::ModuleResult::Pass
//! }
//!
//! ksbh_modules_sdk::register_module!(handle_request, ksbh_modules_sdk::types::ModuleType::Oidc);
//! ```

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

/// Exports a module's FFI entry points for the KSBH host.
///
/// This macro generates two `#[no_mangle]` functions that the host calls:
/// - `get_module_type()`: Returns the module type identifier
/// - `request_filter(ctx)`: The main request processing function
///
/// The macro handles:
/// - Converting the raw FFI `ModuleContext` to the SDK's `RequestContext`
/// - Catching panics from module code and returning HTTP 500
/// - Converting `ModuleResult::Pass` to a null response (continue processing)
/// - Converting `ModuleResult::Stop(Response)` to an FFI response
///
/// # Arguments
///
/// - `$func`: The module's request handler function path (e.g., `handle_request`)
/// - `$type`: The module type expression (e.g., `ModuleType::Oidc`)
///
/// # Example
///
/// ```ignore
/// fn process(ctx: ksbh_modules_sdk::RequestContext<'_>) -> ksbh_modules_sdk::ModuleResult {
///     // ... module logic
///     ksbh_modules_sdk::ModuleResult::Pass
/// }
///
/// ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::Oidc);
/// ```
#[macro_export]
macro_rules! register_module {
    ($func:path, $type:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn get_module_type() -> ksbh_core::modules::abi::ModuleType {
            let module_type = $type;

            match module_type {
                $crate::types::ModuleType::Custom(name) => {
                    static CUSTOM_NAME: ::std::sync::OnceLock<::std::string::String> =
                        ::std::sync::OnceLock::new();
                    let stable_name = CUSTOM_NAME.get_or_init(|| name.to_string());

                    ksbh_core::modules::abi::ModuleType {
                        code: ksbh_core::modules::abi::ModuleTypeCode::Custom,
                        custom_ptr: stable_name.as_ptr(),
                        custom_len: stable_name.len(),
                    }
                }
                _ => module_type.to_ffi(),
            }
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

/// Logs a message at ERROR level via the host's logging infrastructure.
///
/// # Example
///
/// ```ignore
/// log_error!(ctx.logger(), "Failed to validate token: {}", err);
/// ```
#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(0, ::std::format_args!($($arg)*))
    };
}

/// Logs a message at WARN level via the host's logging infrastructure.
#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(1, ::std::format_args!($($arg)*))
    };
}

/// Logs a message at INFO level via the host's logging infrastructure.
#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(2, ::std::format_args!($($arg)*))
    };
}

/// Logs a message at DEBUG level via the host's logging infrastructure.
#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format(3, ::std::format_args!($($arg)*))
    };
}
