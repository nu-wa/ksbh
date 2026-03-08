#[macro_export]
macro_rules! module_error {
    ($ctx:expr, $target:expr, $($arg:tt)*) => {{
        let message = ::std::format!($($arg)*);
        let target_bytes = $target.as_bytes();
        let message_bytes = message.as_bytes();
        unsafe {
            ($ctx.log_fn)(
                $crate::modules::abi::LogLevel::Error,
                target_bytes.as_ptr(),
                target_bytes.len(),
                message_bytes.as_ptr(),
                message_bytes.len(),
            )
        }
    }};
}

#[macro_export]
macro_rules! module_warn {
    ($ctx:expr, $target:expr, $($arg:tt)*) => {{
        let message = ::std::format!($($arg)*);
        let target_bytes = $target.as_bytes();
        let message_bytes = message.as_bytes();
        let result = unsafe {
            ($ctx.log_fn)(
                $crate::modules::abi::LogLevel::Warn,
                target_bytes.as_ptr(),
                target_bytes.len(),
                message_bytes.as_ptr(),
                message_bytes.len(),
            )
        };
        if result != $crate::modules::abi::ModuleResultCode::Ok {
            return $crate::modules::abi::ModuleResultCode::InternalError;
        }
    }};
}

#[macro_export]
macro_rules! module_info {
    ($ctx:expr, $target:expr, $($arg:tt)*) => {{
        let message = ::std::format!($($arg)*);
        let target_bytes = $target.as_bytes();
        let message_bytes = message.as_bytes();
        let result = unsafe {
            ($ctx.log_fn)(
                $crate::modules::abi::LogLevel::Info,
                target_bytes.as_ptr(),
                target_bytes.len(),
                message_bytes.as_ptr(),
                message_bytes.len(),
            )
        };
        if result != $crate::modules::abi::ModuleResultCode::Ok {
            return $crate::modules::abi::ModuleResultCode::InternalError;
        }
    }};
}

#[macro_export]
macro_rules! module_debug {
    ($ctx:expr, $target:expr, $($arg:tt)*) => {{
        let message = ::std::format!($($arg)*);
        let target_bytes = $target.as_bytes();
        let message_bytes = message.as_bytes();
        unsafe {
            ($ctx.log_fn)(
                $crate::modules::abi::LogLevel::Debug,
                target_bytes.as_ptr(),
                target_bytes.len(),
                message_bytes.as_ptr(),
                message_bytes.len(),
            )
        }    }};
}

#[macro_export]
macro_rules! module_trace {
    ($ctx:expr, $target:expr, $($arg:tt)*) => {{
        let message = ::std::format!($($arg)*);
        let target_bytes = $target.as_bytes();
        let message_bytes = message.as_bytes();
        unsafe {
            ($ctx.log_fn)(
                $crate::modules::abi::LogLevel::Trace,
                target_bytes.as_ptr(),
                target_bytes.len(),
                message_bytes.as_ptr(),
                message_bytes.len(),
            )
        }    }};
}
