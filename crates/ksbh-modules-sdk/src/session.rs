use crate::HostModuleCtxHandle;
use ksbh_modules_abi::{
    functions::{HostFnFreeBytes, HostFnSessionGet, HostFnSessionSet, HostFnSessionSharedGet, HostFnSessionSharedSet},
    types::{KSBHBytes, KSBHHostCtxHandle, KSBHHostFnReturn},
};

/// Handle for reading and writing module-specific session data.
///
/// Session data is namespaced by module name and stored per-session (identified by session ID).
/// Data persists across requests and can have optional TTL expiration.
pub struct SessionHandle {
    ctx_handle: HostModuleCtxHandle,
    session_id: [u8; 16],
    get: HostFnSessionGet,
    set: HostFnSessionSet,
    shared_get: HostFnSessionSharedGet,
    shared_set: HostFnSessionSharedSet,
    free: HostFnFreeBytes,
}

impl SessionHandle {
    /// Creates a SessionHandle from FFI function pointers.
    pub fn from_ffi(
        ctx_handle: HostModuleCtxHandle,
        session_id: [u8; 16],
        get: HostFnSessionGet,
        set: HostFnSessionSet,
        free: HostFnFreeBytes,
        shared_get: HostFnSessionSharedGet,
        shared_set: HostFnSessionSharedSet,
    ) -> Self {
        Self {
            ctx_handle,
            session_id,
            get,
            set,
            free,
            shared_get,
            shared_set,
        }
    }

    /// Returns the 16-byte session identifier.
    pub fn session_id(&self) -> [u8; 16] {
        self.session_id
    }

    /// Retrieves session data for the given key.
    ///
    /// Returns `None` if no data exists for the key.
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, crate::ModuleError> {
        let mut host_buffer = KSBHBytes {
            ptr: ::std::ptr::null_mut(),
            len: 0,
        };

        let key = KSBHBytes {
            ptr: key.as_ptr(),
            len: key.len(),
        };

        unsafe {
            match (self.get)(
                KSBHHostCtxHandle {
                    inner: self.ctx_handle,
                },
                key,
                &mut host_buffer,
            ) {
                KSBHHostFnReturn::Success => {
                    if host_buffer.ptr.is_null() || host_buffer.len == 0 {
                        return Err(crate::ModuleError::Host {
                            message: "host returned empty session buffer".to_string(),
                        });
                    }

                    let value =
                        ::std::slice::from_raw_parts(host_buffer.ptr, host_buffer.len).to_vec();

                    (self.free)(
                        KSBHHostCtxHandle {
                            inner: self.ctx_handle,
                        },
                        host_buffer,
                    );

                    Ok(Some(value))
                }
                KSBHHostFnReturn::NotFound => Ok(None),
                KSBHHostFnReturn::BadArgument => Err(crate::ModuleError::Host {
                    message: "session get rejected: bad argument".to_string(),
                }),
                KSBHHostFnReturn::HostFailure => Err(crate::ModuleError::Host {
                    message: "session get failed".to_string(),
                }),
            }
        }
    }
    /// Stores session data for the given key.
    ///
    /// Returns `Ok` if the data was stored successfully.
    /// If no ttl is specified, 0 is sent to the Host, and the Host decides for how long it will
    /// store it. For now we do not allow an indefinite TTL.
    pub fn set(&self, key: &str, data: &[u8], ttl: Option<u64>) -> Result<(), crate::ModuleError> {
        unsafe {
            match (self.set)(
                KSBHHostCtxHandle {
                    inner: self.ctx_handle,
                },
                KSBHBytes {
                    ptr: key.as_ptr(),
                    len: key.len(),
                },
                KSBHBytes {
                    ptr: data.as_ptr(),
                    len: data.len(),
                },
                ttl.unwrap_or(0),
            ) {
                KSBHHostFnReturn::Success => Ok(()),
                KSBHHostFnReturn::BadArgument => Err(crate::ModuleError::Host {
                    message: "session set rejected: bad argument".to_string(),
                }),
                KSBHHostFnReturn::HostFailure => Err(crate::ModuleError::Host {
                    message: "session set failed".to_string(),
                }),
                KSBHHostFnReturn::NotFound => Err(crate::ModuleError::Host {
                    message: "session set failed: host context not found".to_string(),
                }),
            }
        }
    }

    pub fn get_shared(&self, key: &str) -> Result<Option<Vec<u8>>, crate::ModuleError> {
        let key_bytes = KSBHBytes::from_slice(key.as_bytes());
        let mut out = KSBHBytes { ptr: ::std::ptr::null_mut(), len: 0 };

        match unsafe { (self.shared_get)(KSBHHostCtxHandle { inner: self.ctx_handle }, key_bytes, &mut out) } {
            KSBHHostFnReturn::Success => {
                if out.ptr.is_null() || out.len == 0 {
                    return Ok(None);
                }
                let data = unsafe { std::slice::from_raw_parts(out.ptr, out.len).to_vec() };
                unsafe { (self.free)(KSBHHostCtxHandle { inner: self.ctx_handle }, out) };
                Ok(Some(data))
            }
            KSBHHostFnReturn::NotFound => Ok(None),
            other => Err(crate::ModuleError::abi(format!(
                "shared session get failed: {:?}", other
            ))),
        }
    }

    pub fn set_shared(&self, key: &str, data: &[u8], ttl: Option<u64>) -> Result<(), crate::ModuleError> {
        let key_bytes = KSBHBytes::from_slice(key.as_bytes());
        let value_bytes = KSBHBytes::from_slice(data);
        let ttl = ttl.unwrap_or(0);

        match unsafe { (self.shared_set)(KSBHHostCtxHandle { inner: self.ctx_handle }, key_bytes, value_bytes, ttl) } {
            KSBHHostFnReturn::Success => Ok(()),
            KSBHHostFnReturn::BadArgument => Err(crate::ModuleError::abi("invalid argument for shared session set")),
            other => Err(crate::ModuleError::abi(format!(
                "shared session set failed: {:?}", other
            ))),
        }
    }
}
