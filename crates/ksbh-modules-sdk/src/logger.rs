/// Logger for emitting messages via the host's logging infrastructure.
///
/// Log messages are tagged with the module name and forwarded to the host's
/// tracing/logging system.
pub struct Logger {
    log_fn: ksbh_core::modules::abi::log::LogFn,
    mod_name: smol_str::SmolStr,
}

impl Logger {
    /// Creates a new Logger from a host-provided logging function.
    pub fn new(log_fn: ksbh_core::modules::abi::log::LogFn, name: &str) -> Self {
        Self {
            log_fn,
            mod_name: smol_str::SmolStr::from(name),
        }
    }

    /// Logs a message at ERROR level (0).
    pub fn error(&self, msg: &str) {
        self.log(0, msg);
    }

    /// Logs a message at WARN level (1).
    pub fn warn(&self, msg: &str) {
        self.log(1, msg);
    }

    /// Logs a message at INFO level (2).
    pub fn info(&self, msg: &str) {
        self.log(2, msg);
    }

    /// Logs a message at DEBUG level (3).
    pub fn debug(&self, msg: &str) {
        self.log(3, msg);
    }

    /// Logs a message with formatted arguments.
    ///
    /// This is used by the `log_error!`, `log_warn!`, etc. macros.
    pub fn log_with_format(&self, level: u8, args: ::std::fmt::Arguments<'_>) {
        let msg = ::std::format!("{}", args);
        self.log(level, &msg);
    }

    fn log(&self, level: u8, msg: &str) {
        unsafe {
            (self.log_fn)(
                level,
                self.mod_name.as_ptr(),
                self.mod_name.len(),
                msg.as_ptr(),
                msg.len(),
            );
        }
    }
}
