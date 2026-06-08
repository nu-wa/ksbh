//! ModuleKvSlice is used to carry routing and registry config values.

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ModuleKvSlice {
    pub key: bytes::Bytes,
    pub value: bytes::Bytes,
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
