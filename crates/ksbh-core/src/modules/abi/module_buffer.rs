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
