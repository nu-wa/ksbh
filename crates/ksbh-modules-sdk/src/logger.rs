use crate::HostModuleCtxHandle;
use ksbh_modules_abi::{
    functions::HostFnLog,
    types::{KSBHBytes, KSBHHostCtxHandle, LogLevel},
};

/// Logger for emitting messages via the host's logging infrastructure.
///
/// Log messages are tagged with the module name and forwarded to the host's
/// tracing/logging system.
pub struct Logger {
    ctx_handle: HostModuleCtxHandle,
    log_fn: HostFnLog,
}

impl Logger {
    /// Creates a new Logger from a host-provided logging function.
    pub fn new(ctx_handle: crate::HostModuleCtxHandle, log_fn: HostFnLog) -> Self {
        Self { log_fn, ctx_handle }
    }

    pub fn error(&self, msg: &str) {
        self.log(LogLevel::Error, msg);
    }

    pub fn warn(&self, msg: &str) {
        self.log(LogLevel::Warn, msg);
    }

    pub fn info(&self, msg: &str) {
        self.log(LogLevel::Info, msg);
    }

    pub fn debug(&self, msg: &str) {
        self.log(LogLevel::Debug, msg);
    }

    /// Logs a message with formatted arguments.
    ///
    /// This is used by the `log_error!`, `log_warn!`, etc. macros.
    pub fn log_with_format(&self, level: LogLevel, args: ::std::fmt::Arguments<'_>) {
        let msg = ::std::format!("{}", args);
        self.log(level, &msg);
    }

    fn log(&self, level: LogLevel, msg: &str) {
        unsafe {
            (self.log_fn)(
                KSBHHostCtxHandle {
                    inner: self.ctx_handle,
                },
                level,
                KSBHBytes {
                    ptr: msg.as_ptr(),
                    len: msg.len(),
                },
            );
        }
    }
}
