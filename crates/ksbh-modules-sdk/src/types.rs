pub enum ModuleType {
    Oidc,
    Pow,
    RateLimit,
    HttpToHttps,
    RobotsTxt,
    Custom(smol_str::SmolStr),
}

impl ModuleType {
    pub fn to_ffi(&self) -> ksbh_core::modules::abi::ModuleType {
        match self {
            ModuleType::Oidc => ksbh_core::modules::abi::ModuleType {
                code: ksbh_core::modules::abi::ModuleTypeCode::OIDC,
                custom_ptr: ::std::ptr::null(),
                custom_len: 0,
            },
            ModuleType::Pow => ksbh_core::modules::abi::ModuleType {
                code: ksbh_core::modules::abi::ModuleTypeCode::POW,
                custom_ptr: ::std::ptr::null(),
                custom_len: 0,
            },
            ModuleType::RateLimit => ksbh_core::modules::abi::ModuleType {
                code: ksbh_core::modules::abi::ModuleTypeCode::RateLimit,
                custom_ptr: ::std::ptr::null(),
                custom_len: 0,
            },
            ModuleType::HttpToHttps => ksbh_core::modules::abi::ModuleType {
                code: ksbh_core::modules::abi::ModuleTypeCode::HttpToHttps,
                custom_ptr: ::std::ptr::null(),
                custom_len: 0,
            },
            ModuleType::RobotsTxt => ksbh_core::modules::abi::ModuleType {
                code: ksbh_core::modules::abi::ModuleTypeCode::RobotsDotTXT,
                custom_ptr: ::std::ptr::null(),
                custom_len: 0,
            },
            ModuleType::Custom(name) => ksbh_core::modules::abi::ModuleType {
                code: ksbh_core::modules::abi::ModuleTypeCode::Custom,
                custom_ptr: name.as_ptr(),
                custom_len: name.len(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_type_oidc() {
        let mtype = ModuleType::Oidc;
        let ffi = mtype.to_ffi();
        assert_eq!(ffi.code, ksbh_core::modules::abi::ModuleTypeCode::OIDC);
    }

    #[test]
    fn test_module_type_pow() {
        let mtype = ModuleType::Pow;
        let ffi = mtype.to_ffi();
        assert_eq!(ffi.code, ksbh_core::modules::abi::ModuleTypeCode::POW);
    }

    #[test]
    fn test_module_type_rate_limit() {
        let mtype = ModuleType::RateLimit;
        let ffi = mtype.to_ffi();
        assert_eq!(ffi.code, ksbh_core::modules::abi::ModuleTypeCode::RateLimit);
    }

    #[test]
    fn test_module_type_http_to_https() {
        let mtype = ModuleType::HttpToHttps;
        let ffi = mtype.to_ffi();
        assert_eq!(
            ffi.code,
            ksbh_core::modules::abi::ModuleTypeCode::HttpToHttps
        );
    }

    #[test]
    fn test_module_type_robots_txt() {
        let mtype = ModuleType::RobotsTxt;
        let ffi = mtype.to_ffi();
        assert_eq!(
            ffi.code,
            ksbh_core::modules::abi::ModuleTypeCode::RobotsDotTXT
        );
    }

    #[test]
    fn test_module_type_custom() {
        let mtype = ModuleType::Custom(smol_str::SmolStr::new("my-module"));
        let ffi = mtype.to_ffi();
        assert_eq!(ffi.code, ksbh_core::modules::abi::ModuleTypeCode::Custom);
    }
}
