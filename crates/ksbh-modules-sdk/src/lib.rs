//! SDK for building FFI modules for the KSBH reverse proxy.
//!
//! This crate provides a convenient Rust API for building dynamically-loaded modules
//! that interface with KSBH via the FFI ABI defined in `ksbh_core::modules::runtime`.
//!
//! # Core Components
//!
//! - [`context::ModuleContext`] - Safe wrapper around the raw module context,
//!   providing access to request data, headers, session storage, and reputation
//! - [`result::ModuleResult`] - Return type for module request handlers
//!   (`Pass` to continue, `Stop(Response)` to return immediately)
//! - [`error::ModuleError`] - Error type with convenience constructors for
//!   common HTTP status codes
//! - [`session::SessionHandle`] - Read/write session data with TTL support
//! - [`ReputationHandle`] - Report reputation to the host (score tracking)
//! - [`logger::Logger`] - Log messages via the host's logging infrastructure
//!
//! # Module Entry Point
//!
//! Modules must implement a `process(ctx: ModuleContext) -> Result<ModuleResult, ModuleError>`
//! function and use the [`export_module!`] macro to export the required FFI functions.
//!
//! # Example
//!
//! ```ignore
//! fn handle_request(
//!     mut ctx: ksbh_modules_sdk::ModuleContext<'_>,
//! ) -> ksbh_modules_sdk::ModuleResult {
//!     let path = ctx.request().path();
//!     // ... process request
//!     ksbh_modules_sdk::ModuleResult::Pass
//! }
//!
//! ksbh_modules_sdk::export_module!(
//!     handle_request,
//!     ksbh_modules_sdk::module_definition!(
//!         ksbh_modules_sdk::abi::prelude::KSBHModuleKind::OIDC,
//!         [ksbh_modules_sdk::abi::prelude::KSBHModuleStage::Request]
//!     )
//! );
//! ```

pub mod context;
pub mod error;
pub mod logger;
pub mod metrics;
pub mod result;
pub mod session;
pub mod utils;

pub mod owned_module_response;
pub mod responses;

pub use context::{ModuleContext, RequestInfo};
pub use error::ModuleError;
pub use metrics::ReputationHandle;
pub use result::ModuleResult;
pub use responses::{empty_response, plain_text_response, redirect_response, text_response};

pub use ksbh_modules_abi as abi;
pub type HostSignalKind = abi::functions::KSBHHostSignalKind;

pub type HostModuleCtxHandle = u64;
pub type RequestStage = abi::types::KSBHModuleStage;
pub type RequestResult = Result<ModuleResult, ModuleError>;

#[macro_export]
macro_rules! export_module {
    ($func:path, $module_info:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn module_descriptor()
        -> $crate::abi::module_descriptor::ModuleDescriptor {
            $crate::abi::module_descriptor::ModuleDescriptor {
                magic: $crate::abi::version::KSBH_MAGIC,
                abi_version: $crate::abi::version::KSBH_ABI_VERSION,
                info: $module_info,
                handle_request_fn: handle_request,
                free_module_response_fn: free_module_response,
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn handle_request(
            stage: $crate::abi::prelude::KSBHModuleStage,
            ctx: *const $crate::abi::prelude::ModuleContext,
        ) -> *const $crate::abi::prelude::ModuleResponse {
            if ctx.is_null() {
                return ::std::ptr::null();
            }

            let ctx_ref = unsafe { &*ctx };

            let mod_ctx = match $crate::ModuleContext::try_from(ctx_ref) {
                Ok(mod_ctx) => mod_ctx,
                Err(_) => {
                    return $crate::owned_module_response::SdkOwnedModuleResponse::new_empty(Some(
                        $crate::abi::prelude::KSBHModuleDecision::Error,
                    ))
                    .to_abi();
                }
            };

            match $func(stage, mod_ctx) {
                Ok($crate::ModuleResult::Pass) => {
                    $crate::owned_module_response::SdkOwnedModuleResponse::new_empty(None)
                }
                Ok($crate::ModuleResult::Stop(resp)) => {
                    $crate::owned_module_response::SdkOwnedModuleResponse::from_http_response(
                        Some($crate::abi::prelude::KSBHModuleDecision::Stop),
                        resp,
                    )
                }
                Ok($crate::ModuleResult::Error(resp)) => {
                    $crate::owned_module_response::SdkOwnedModuleResponse::from_http_response(
                        Some($crate::abi::prelude::KSBHModuleDecision::Error),
                        resp,
                    )
                }
                Err(_) => $crate::owned_module_response::SdkOwnedModuleResponse::new_empty(Some(
                    $crate::abi::prelude::KSBHModuleDecision::Error,
                )),
            }
            .to_abi()
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn free_module_response(
            ctx: *const $crate::abi::prelude::ModuleContext,
            module_response: *mut $crate::abi::prelude::ModuleResponse,
        ) {
            let _ = ctx;
            unsafe {
                $crate::owned_module_response::free_module_response_owned(module_response);
            }
        }
    };
}

#[macro_export]
macro_rules! module_definition {
      ($kind:expr, [$($stage:expr),* $(,)?]) => {{
          static MODULE_STAGES: &[$crate::RequestStage] = &[$($stage),*];
          static MODULE_NAME: &str = "";

          $crate::abi::prelude::ModuleInfo {
              kind: $kind,
              name: $crate::abi::prelude::KSBHString {
                  inner: $crate::abi::prelude::KSBHBytes {
                      ptr: MODULE_NAME.as_ptr(),
                      len: MODULE_NAME.len(),
                  },
              },
              registered_stages: $crate::abi::prelude::KSBHModuleStages {
                  ptr: MODULE_STAGES.as_ptr(),
                  len: MODULE_STAGES.len(),
              },
          }
      }};

      ($name:literal, $kind:expr, [$($stage:expr),* $(,)?]) => {{
          static MODULE_STAGES: &[$crate::RequestStage] = &[$($stage),*];
          static MODULE_NAME: &str = $name;

          $crate::abi::prelude::ModuleInfo {
              kind: $kind,
              name: $crate::abi::prelude::KSBHString {
                  inner: $crate::abi::prelude::KSBHBytes {
                      ptr: MODULE_NAME.as_ptr(),
                      len: MODULE_NAME.len(),
                  },
              },
              registered_stages: $crate::abi::prelude::KSBHModuleStages {
                  ptr: MODULE_STAGES.as_ptr(),
                  len: MODULE_STAGES.len(),
              },
          }
      }};
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
        $logger.log_with_format($crate::abi::types::LogLevel::Error, ::std::format_args!($($arg)*))
    };
}

/// Logs a message at WARN level via the host's logging infrastructure.
#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format($crate::abi::types::LogLevel::Warn, ::std::format_args!($($arg)*))
    };
}

/// Logs a message at INFO level via the host's logging infrastructure.
#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format($crate::abi::types::LogLevel::Info, ::std::format_args!($($arg)*))
    };
}

/// Logs a message at DEBUG level via the host's logging infrastructure.
#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_with_format($crate::abi::types::LogLevel::Debug, ::std::format_args!($($arg)*))
    };
}
