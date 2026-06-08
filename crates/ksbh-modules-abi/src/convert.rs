//! Safe conversion helpers between ABI types and Rust types.
//!
//! These are shared utilities used by both the host (ksbh-core) and the
//! SDK (ksbh-modules-sdk) to marshal data across the FFI boundary.

use crate::prelude::{KSBHBytes, KSBHString, KSBHKVString};

impl KSBHBytes {
    /// Decode ABI bytes into a Rust byte slice.
    ///
    /// Returns `&[]` for null pointers or zero-length data.
    /// The caller must ensure `ptr` is valid for the returned lifetime.
    pub fn as_slice(&self) -> &[u8] {
        if self.ptr.is_null() || self.len == 0 {
            return &[];
        }
        unsafe { ::std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Create `KSBHBytes` from a Rust byte slice.
    ///
    /// The returned value borrows from `slice` — the caller must ensure
    /// `slice` outlives usage across the FFI boundary.
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
        }
    }
}

impl KSBHString {
    /// Decode an ABI string into a Rust `&str`.
    ///
    /// Returns `""` for null pointers or zero-length data.
    /// Returns an empty string on invalid UTF-8.
    pub fn as_str(&self) -> &str {
        let bytes = self.inner.as_slice();
        ::std::str::from_utf8(bytes).unwrap_or("")
    }

    /// Create a `KSBHString` from a Rust `&str`.
    ///
    /// The returned value borrows from `s`.
    pub fn from_str(s: &str) -> Self {
        Self {
            inner: KSBHBytes::from_slice(s.as_bytes()),
        }
    }
}

/// Build a `Vec<KSBHKVString>` from string key-value pairs.
///
/// This is used by the host to construct the config and query-param
/// slices passed to modules.
pub fn build_kv_entries<'a, I>(pairs: I) -> Vec<KSBHKVString>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    pairs
        .into_iter()
        .map(|(key, value)| KSBHKVString {
            key: KSBHString::from_str(key),
            value: KSBHString::from_str(value),
        })
        .collect()
}
