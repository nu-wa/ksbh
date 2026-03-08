pub struct Logger {
    log_fn: ksbh_core::modules::abi::log::LogFn,
    mod_name: smol_str::SmolStr,
}

impl Logger {
    pub fn new(log_fn: ksbh_core::modules::abi::log::LogFn, name: &str) -> Self {
        Self {
            log_fn,
            mod_name: smol_str::SmolStr::from(name),
        }
    }

    pub fn error(&self, msg: &str) {
        self.log(0, msg);
    }

    pub fn warn(&self, msg: &str) {
        self.log(1, msg);
    }

    pub fn info(&self, msg: &str) {
        self.log(2, msg);
    }

    pub fn debug(&self, msg: &str) {
        self.log(3, msg);
    }

    #[doc(hidden)]
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
