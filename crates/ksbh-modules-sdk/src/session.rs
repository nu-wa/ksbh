/// Handle for reading and writing module-specific session data.
///
/// Session data is namespaced by module name and stored per-session (identified by session ID).
/// Data persists across requests and can have optional TTL expiration.
pub struct SessionHandle {
    session_id: [u8; 16],
    module_name: smol_str::SmolStr,
    get: ksbh_core::modules::abi::SessionGetFn,
    set: ksbh_core::modules::abi::SessionSetFn,
    set_ttl: ksbh_core::modules::abi::SessionSetWithTtlFn,
    free: ksbh_core::modules::abi::SessionFreeFn,
}

impl SessionHandle {
    /// Creates a SessionHandle from FFI function pointers.
    pub fn from_ffi(
        session_id: [u8; 16],
        module_name: smol_str::SmolStr,
        get: ksbh_core::modules::abi::SessionGetFn,
        set: ksbh_core::modules::abi::SessionSetFn,
        set_ttl: ksbh_core::modules::abi::SessionSetWithTtlFn,
        free: ksbh_core::modules::abi::SessionFreeFn,
    ) -> Self {
        Self {
            session_id,
            module_name,
            get,
            set,
            set_ttl,
            free,
        }
    }

    /// Returns the 16-byte session identifier.
    pub fn session_id(&self) -> [u8; 16] {
        self.session_id
    }

    /// Retrieves session data for the given key.
    ///
    /// Returns `None` if no data exists for the key.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        let found = unsafe {
            (self.get)(
                self.session_id.as_ptr(),
                self.module_name.as_ptr(),
                self.module_name.len(),
                key.as_ptr(),
                key.len(),
                &mut out_ptr,
                &mut out_len,
            )
        };
        if !found || out_ptr.is_null() || out_len == 0 {
            return None;
        }
        let data = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
        unsafe {
            (self.free)(
                self.module_name.as_ptr(),
                self.module_name.len(),
                out_ptr,
                out_len,
            )
        };
        Some(data)
    }
    /// Stores session data for the given key.
    ///
    /// Returns `true` if the data was stored successfully.
    /// Data persists indefinitely (no TTL) unless `set_with_ttl` is used.
    pub fn set(&self, key: &str, data: &[u8]) -> bool {
        unsafe {
            (self.set)(
                self.session_id.as_ptr(),
                self.module_name.as_ptr(),
                self.module_name.len(),
                key.as_ptr(),
                key.len(),
                data.as_ptr(),
                data.len(),
            )
        }
    }

    /// Stores session data with a TTL (time-to-live) in seconds.
    ///
    /// Returns `true` if the data was stored successfully.
    /// After `ttl_secs` seconds, the data will expire and `get` will return `None`.
    pub fn set_with_ttl(&self, key: &str, data: &[u8], ttl_secs: u64) -> bool {
        unsafe {
            (self.set_ttl)(
                self.session_id.as_ptr(),
                self.module_name.as_ptr(),
                self.module_name.len(),
                key.as_ptr(),
                key.len(),
                data.as_ptr(),
                data.len(),
                ttl_secs,
            )
        }
    }
}
