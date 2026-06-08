use ::std::collections::HashMap;
use http::{HeaderMap, HeaderValue};
use ksbh_modules_abi::types::*;

pub fn abi_bytes_slice_to_sdk<'ctx>(value: &'ctx KSBHBytes) -> anyhow::Result<&'ctx [u8]> {
    Ok(value.as_slice())
}

pub fn abi_string_to_sdk<'ctx>(value: &'ctx KSBHString) -> anyhow::Result<&'ctx str> {
    Ok(value.as_str())
}

pub fn abi_string_to_sdk_vec<'ctx>(
    value: &'ctx KSBHSliceStrings,
) -> anyhow::Result<Vec<&'ctx str>> {
    unsafe {
        let values = ::std::slice::from_raw_parts(value.ptr, value.len);

        let mut result = Vec::with_capacity(values.len());

        for v in values.iter() {
            result.push(abi_string_to_sdk(v)?);
        }

        Ok(result)
    }
}

pub fn abi_string_to_sdk_hashmap<'ctx>(
    value: &'ctx KSBHSliceKVStrings,
) -> anyhow::Result<HashMap<&'ctx str, &'ctx str>> {
    unsafe {
        let values = ::std::slice::from_raw_parts(value.ptr, value.len);

        let mut result = HashMap::with_capacity(values.len());

        for v in values.iter() {
            result.insert(abi_string_to_sdk(&v.key)?, abi_string_to_sdk(&v.value)?);
        }

        Ok(result)
    }
}

pub fn abi_headers_to_sdk<'ctx>(value: &'ctx KSBHSliceKVStrings) -> anyhow::Result<HeaderMap> {
    unsafe {
        let values = ::std::slice::from_raw_parts(value.ptr, value.len);

        let mut result = HeaderMap::new();

        for v in values.iter() {
            result.insert(
                abi_string_to_sdk(&v.key)?,
                HeaderValue::from_bytes(abi_bytes_slice_to_sdk(&v.value.inner)?)?,
            );
        }

        Ok(result)
    }
}
