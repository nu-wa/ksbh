#[repr(C)]
pub struct ModuleResponse {
    pub status_code: u16,
    pub headers: *const u8,
    pub headers_size: usize,
    pub body: *const u8,
    pub body_size: usize,
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
            headers: ::std::ptr::null(),
            headers_size: 0,
            body: ::std::ptr::null(),
            body_size: 0,
        }
    }

    pub fn is_null(&self) -> bool {
        self.headers.is_null() && self.body.is_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_response_null() {
        let response = ModuleResponse::null();
        assert_eq!(response.status_code, 0);
        assert!(response.headers.is_null());
        assert!(response.body.is_null());
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
