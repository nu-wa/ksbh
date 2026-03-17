//! ModuleBuffer and its OwnedModuleBuffer are used to send and
//! receieve, respectively, data from the "host" to the module.

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ModuleBuffer {
    pub data: bytes::Bytes,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ModuleKvSlice {
    pub key: bytes::Bytes,
    pub value: bytes::Bytes,
}

impl ModuleBuffer {
    pub fn as_str(&self) -> Option<&str> {
        if self.data.is_empty() {
            return None;
        }
        ::std::str::from_utf8(&self.data).ok()
    }

    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn from_ref(src: &str) -> Self {
        Self {
            data: bytes::Bytes::copy_from_slice(src.as_bytes()),
        }
    }

    pub fn from_ref_bytes(src: &[u8]) -> Self {
        Self {
            data: bytes::Bytes::copy_from_slice(src),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn null() -> Self {
        Self {
            data: bytes::Bytes::new(),
        }
    }
}

impl ModuleKvSlice {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: bytes::Bytes::copy_from_slice(key.as_bytes()),
            value: bytes::Bytes::copy_from_slice(value.as_bytes()),
        }
    }

    pub fn key_str(&self) -> &str {
        ::std::str::from_utf8(&self.key).unwrap_or_default()
    }

    pub fn value_str(&self) -> &str {
        ::std::str::from_utf8(&self.value).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_buffer_from_ref() {
        let buffer = ModuleBuffer::from_ref("hello");
        assert_eq!(buffer.data.len(), 5);
        assert_eq!(buffer.as_str(), Some("hello"));
    }

    #[test]
    fn test_module_buffer_null() {
        let buffer = ModuleBuffer::null();
        assert!(buffer.data.is_empty());
        assert_eq!(buffer.data.len(), 0);
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
        assert_eq!(kv.key.len(), 3);
        assert_eq!(kv.value.len(), 5);
    }

    #[test]
    fn test_module_kv_slice_empty() {
        let kv = ModuleKvSlice::new("", "");
        assert_eq!(kv.key.len(), 0);
        assert_eq!(kv.value.len(), 0);
    }
}
