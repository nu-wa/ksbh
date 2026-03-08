//! ModuleBuffer and its OwnedModuleBuffer are used to send and
//! receieve, respectively, data from the "host" to the module.

#[repr(C)]
#[derive(Debug)]
pub struct ModuleBuffer {
    pub data: *const u8,
    pub size: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct ModuleKvSlice {
    pub key: *const u8,
    pub key_len: usize,
    pub value: *const u8,
    pub value_len: usize,
}

// Represents memory created by a module that will be freed by the host
#[repr(C)]
pub struct OwnedModuleBuffer {
    pub buffer: ModuleBuffer,
    pub free_ptr: FreeOwnedModuleBuffer,
}

pub type FreeOwnedModuleBuffer = unsafe extern "C" fn(ModuleBuffer);

unsafe impl Send for ModuleKvSlice {}
unsafe impl Sync for ModuleKvSlice {}

unsafe impl Send for ModuleBuffer {}
unsafe impl Sync for ModuleBuffer {}

unsafe impl Send for OwnedModuleBuffer {}
unsafe impl Sync for OwnedModuleBuffer {}

impl ModuleBuffer {
    pub fn as_str(&self) -> Option<&str> {
        if self.data.is_null() {
            return None;
        }

        unsafe {
            let slice = ::std::slice::from_raw_parts(self.data, self.size);

            ::std::str::from_utf8(slice).ok()
        }
    }

    pub fn empty(&self) -> bool {
        self.data.is_null() || self.size == 0
    }

    pub fn from_ref(src: &str) -> Self {
        Self {
            data: src.as_ptr(),
            size: src.len(),
        }
    }

    pub fn null() -> Self {
        Self {
            data: ::std::ptr::null(),
            size: 0,
        }
    }
}

impl ModuleKvSlice {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.as_ptr(),
            key_len: key.len(),
            value: value.as_ptr(),
            value_len: value.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_buffer_from_ref() {
        let buffer = ModuleBuffer::from_ref("hello");
        assert_eq!(buffer.size, 5);
        assert_eq!(buffer.as_str(), Some("hello"));
    }

    #[test]
    fn test_module_buffer_null() {
        let buffer = ModuleBuffer::null();
        assert!(buffer.data.is_null());
        assert_eq!(buffer.size, 0);
    }

    #[test]
    fn test_module_buffer_empty() {
        let buffer = ModuleBuffer::null();
        assert!(buffer.empty());
    }

    #[test]
    fn test_module_buffer_not_empty() {
        let buffer = ModuleBuffer::from_ref("test");
        assert!(!buffer.empty());
    }

    #[test]
    fn test_module_buffer_as_str_none_on_null() {
        let buffer = ModuleBuffer::null();
        assert_eq!(buffer.as_str(), None);
    }

    #[test]
    fn test_module_kv_slice_new() {
        let kv = ModuleKvSlice::new("key", "value");
        assert_eq!(kv.key_len, 3);
        assert_eq!(kv.value_len, 5);
    }

    #[test]
    fn test_module_kv_slice_empty() {
        let kv = ModuleKvSlice::new("", "");
        assert_eq!(kv.key_len, 0);
        assert_eq!(kv.value_len, 0);
    }
}
