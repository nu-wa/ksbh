pub mod error;
pub mod host_functions;
pub mod log;
pub mod macros;
pub mod module_buffer;
pub mod module_context;
pub mod module_host;
pub mod module_instance;
pub mod module_request_info;
pub mod module_response;

pub use module_buffer::{ModuleBuffer, ModuleKvSlice, OwnedModuleBuffer};
pub use module_context::ModuleContext;
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

pub type MetricsIncrementGoodFn = unsafe extern "C" fn(
    client_ip: *const u8,
    client_ip_len: usize,
    user_agent: *const u8,
    user_agent_len: usize,
) -> bool;

pub type MetricsGetHitsFn = unsafe extern "C" fn(
    client_ip: *const u8,
    client_ip_len: usize,
    user_agent: *const u8,
    user_agent_len: usize,
    out_good: *mut u32,
    out_bad: *mut u32,
) -> bool;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModuleTypeCode {
    OIDC = 0,
    POW = 1,
    RateLimit = 2,
    HttpToHttps = 3,
    RobotsDotTXT = 4,
    Custom = 5,
}

#[repr(C)]
#[derive(Clone)]
pub struct ModuleType {
    pub code: ModuleTypeCode,
    pub custom_ptr: *const u8,
    pub custom_len: usize,
}

pub type ModuleEntryFn =
    unsafe extern "C" fn(ctx: *const ModuleContext<'_>) -> *const ModuleResponse;

pub type ModuleResponseFreeFn = unsafe extern "C" fn(*const ModuleResponse);

pub type ModuleGetTypeFn = unsafe extern "C" fn() -> ModuleType;

pub struct ModuleSession<'a> {
    ctx: &'a ModuleContext<'a>,
}

impl<'a> ModuleSession<'a> {
    pub fn from_ctx(ctx: &'a ModuleContext<'a>) -> Self {
        Self { ctx }
    }

    pub fn get_header(&self, name: &str) -> Option<&'a str> {
        for header in self.ctx.headers.iter() {
            let key = unsafe { ::std::slice::from_raw_parts(header.key, header.key_len) };
            let key_str = ::std::str::from_utf8(key).ok()?;

            if key_str.eq_ignore_ascii_case(name) {
                let value = unsafe { ::std::slice::from_raw_parts(header.value, header.value_len) };
                return ::std::str::from_utf8(value).ok();
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
        let key_bytes = key.as_bytes();
        for entry in self.ctx.config.iter() {
            let k = unsafe { ::std::slice::from_raw_parts(entry.key, entry.key_len) };
            if k == key_bytes {
                let v = unsafe { ::std::slice::from_raw_parts(entry.value, entry.value_len) };
                return ::std::str::from_utf8(v).ok();
            }
        }
        None
    }

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
