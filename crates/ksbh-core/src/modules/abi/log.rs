pub type LogFn = unsafe extern "C" fn(
    level: u8,
    target: *const u8,
    target_len: usize,
    message: *const u8,
    message_len: usize,
) -> u8;

pub const LOG_LEVEL_ERROR: u8 = 0;
pub const LOG_LEVEL_WARN: u8 = 1;
pub const LOG_LEVEL_INFO: u8 = 2;
pub const LOG_LEVEL_DEBUG: u8 = 3;
