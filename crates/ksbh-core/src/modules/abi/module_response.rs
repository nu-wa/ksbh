#[repr(C)]
pub struct ModuleResponse {
    pub status_code: u16,
    pub headers_ptr: *const super::ModuleKvSlice,
    pub headers_len: usize,
    pub body: bytes::Bytes,
}

/// Stupid name i hate it
#[repr(C)]
pub enum ModuleResponseResult {
    Ok = 0,
    Err = 1,
}

impl ModuleResponse {
    pub fn null() -> Self {
        Self {
            status_code: 0,
            headers_ptr: ::std::ptr::null(),
            headers_len: 0,
            body: bytes::Bytes::new(),
        }
    }

    pub fn is_null(&self) -> bool {
        self.headers_ptr.is_null() && self.body.is_empty()
    }

    pub fn headers_slice(&self) -> &[super::ModuleKvSlice] {
        if self.headers_ptr.is_null() || self.headers_len == 0 {
            return &[];
        }
        unsafe { ::std::slice::from_raw_parts(self.headers_ptr, self.headers_len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_response_null() {
        let response = ModuleResponse::null();
        assert_eq!(response.status_code, 0);
        assert!(response.headers_ptr.is_null());
        assert!(response.body.is_empty());
    }

    #[test]
    fn test_module_response_is_null() {
        let response = ModuleResponse::null();
        assert!(response.is_null());
    }

    #[test]
    fn test_module_response_result_ok() {
        let result = ModuleResponseResult::Ok;
        assert_eq!(result as i32, 0);
    }

    #[test]
    fn test_module_response_result_err() {
        let result = ModuleResponseResult::Err;
        assert_eq!(result as i32, 1);
    }
}
