pub mod error;
pub mod host_functions;
pub mod log;
pub mod macros;
pub mod module_buffer;
pub mod module_context;
pub mod module_host;
pub mod module_instance;
pub mod module_request_context;
pub mod module_request_info;
pub mod module_response;

pub use module_buffer::{ModuleBuffer, ModuleKvSlice};
pub use module_context::ModuleContext;
pub use module_request_context::{ModuleCallParams, ModuleRequestContext};
pub use module_request_info::{QueryParams, RequestInfo};
pub use module_response::{ModuleResponse, ModuleResponseResult};

#[allow(improper_ctypes_definitions)]
pub type ModuleLogFn = unsafe extern "C" fn(
    level: u8,
    target: *const u8,
    target_len: usize,
    message: *const u8,
    message_len: usize,
) -> u8;

pub type SessionFreeFn = unsafe extern "C" fn(
    module_name: *const u8,
    module_name_len: usize,
    ptr: *const u8,
    len: usize,
);

pub type SessionGetFn = unsafe extern "C" fn(
    session_id: *const u8,
    module_name: *const u8,
    module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> bool;

pub type SessionSetFn = unsafe extern "C" fn(
    session_id: *const u8,
    module_name: *const u8,
    module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    data_ptr: *const u8,
    data_len: usize,
) -> bool;

pub type SessionSetWithTtlFn = unsafe extern "C" fn(
    session_id: *const u8,
    module_name: *const u8,
    module_name_len: usize,
    data_key: *const u8,
    data_key_len: usize,
    data_ptr: *const u8,
    data_len: usize,
    ttl_secs: u64,
) -> bool;

pub type MetricsGoodBoyFn =
    unsafe extern "C" fn(metrics_key: *const u8, metrics_key_len: usize) -> bool;

pub type MetricsGetScoreFn =
    unsafe extern "C" fn(metrics_key: *const u8, metrics_key_len: usize) -> u64;

/// FFI enum for built-in module types.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModuleTypeCode {
    /// OpenID Connect authentication module
    OIDC = 0,
    /// Proof-of-work challenge module
    POW = 1,
    /// Rate limiting module
    RateLimit = 2,
    /// HTTP to HTTPS redirect module
    HttpToHttps = 3,
    /// robots.txt handling module
    RobotsDotTXT = 4,
    /// Custom module with user-defined type string
    Custom = 5,
}

/// FFI struct representing a module type, supporting both built-in and custom types.
#[repr(C)]
#[derive(Clone)]
pub struct ModuleType {
    /// Built-in module type code
    pub code: ModuleTypeCode,
    /// Pointer to custom type string (used when `code` is `Custom`)
    pub custom_ptr: *const u8,
    /// Length of custom type string
    pub custom_len: usize,
}

/// FFI entry point function type for module request filtering.
/// Modules must export a function with this signature named `request_filter`.
pub type ModuleEntryFn =
    unsafe extern "C" fn(ctx: *const ModuleContext<'_>) -> *const ModuleResponse;

pub type ModuleResponseFreeFn = unsafe extern "C" fn(
    headers_ptr: *const ModuleKvSlice,
    headers_len: usize,
    body_ptr: *const u8,
    body_len: usize,
);

pub type ModuleGetTypeFn = unsafe extern "C" fn() -> ModuleType;

/// Session access for modules.
/// Provides methods to read/write session-scoped data and access request information.
pub struct ModuleSession<'a> {
    ctx: &'a ModuleContext<'a>,
}

impl<'a> ModuleSession<'a> {
    pub fn from_ctx(ctx: &'a ModuleContext<'a>) -> Self {
        Self { ctx }
    }

    pub fn get_header(&self, name: &str) -> Option<&'a str> {
        for header in self.ctx.headers.iter() {
            if header.key.as_ref().eq_ignore_ascii_case(name.as_bytes()) {
                return ::std::str::from_utf8(&header.value).ok();
            }
        }
        None
    }

    pub fn body(&self) -> &'a [u8] {
        self.ctx.body
    }

    pub fn request_info(&self) -> &'a module_request_info::RequestInfo {
        self.ctx.req_info
    }

    pub fn get_config(&self, key: &str) -> Option<&'a str> {
        for entry in self.ctx.config.iter() {
            if entry.key.as_ref() == key.as_bytes() {
                return ::std::str::from_utf8(&entry.value).ok();
            }
        }
        None
    }

    /// Retrieves session data for the given module and key.
    /// Returns `None` if no data exists or if the session has expired.
    pub fn get_session_data(&self, module_name: &str, data_key: &str) -> Option<Vec<u8>> {
        let mut out_ptr: *const u8 = ::std::ptr::null();
        let mut out_len: usize = 0;

        let found = unsafe {
            (self.ctx.session_get_fn)(
                self.ctx.session_id.as_ptr(),
                module_name.as_ptr(),
                module_name.len(),
                data_key.as_ptr(),
                data_key.len(),
                &mut out_ptr,
                &mut out_len,
            )
        };

        if !found || out_ptr.is_null() || out_len == 0 {
            return None;
        }

        let data = unsafe { ::std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
        unsafe {
            (self.ctx.session_free_fn)(module_name.as_ptr(), module_name.len(), out_ptr, out_len)
        };

        Some(data)
    }

    /// Stores session data for the given module and key.
    /// Returns `true` on success, `false` on failure.
    pub fn set_session_data(&self, module_name: &str, data_key: &str, data: &[u8]) -> bool {
        unsafe {
            (self.ctx.session_set_fn)(
                self.ctx.session_id.as_ptr(),
                module_name.as_ptr(),
                module_name.len(),
                data_key.as_ptr(),
                data_key.len(),
                data.as_ptr(),
                data.len(),
            )
        }
    }

    /// Stores session data with a TTL (time-to-live) expiration.
    /// Returns `true` on success, `false` on failure.
    pub fn set_session_data_with_ttl(
        &self,
        module_name: &str,
        data_key: &str,
        data: &[u8],
        ttl_secs: u64,
    ) -> bool {
        unsafe {
            (self.ctx.session_set_with_ttl_fn)(
                self.ctx.session_id.as_ptr(),
                module_name.as_ptr(),
                module_name.len(),
                data_key.as_ptr(),
                data_key.len(),
                data.as_ptr(),
                data.len(),
                ttl_secs,
            )
        }
    }
}

impl ModuleContext<'_> {
    pub fn session(&self) -> ModuleSession<'_> {
        ModuleSession::from_ctx(self)
    }
}

impl TryFrom<ModuleType> for crate::modules::ModuleConfigurationType {
    type Error = std::string::FromUtf8Error;

    fn try_from(value: ModuleType) -> Result<Self, Self::Error> {
        match value.code {
            ModuleTypeCode::Custom => {
                let t = unsafe { ::std::slice::from_raw_parts(value.custom_ptr, value.custom_len) };
                let s = String::from_utf8(t.to_vec())?;
                Ok(crate::modules::ModuleConfigurationType::Custom(s))
            }
            ModuleTypeCode::HttpToHttps => Ok(Self::HttpToHttps),
            ModuleTypeCode::OIDC => Ok(Self::OIDC),
            ModuleTypeCode::POW => Ok(Self::POW),
            ModuleTypeCode::RateLimit => Ok(Self::RateLimit),
            ModuleTypeCode::RobotsDotTXT => Ok(Self::RobotsDotTXT),
        }
    }
}
