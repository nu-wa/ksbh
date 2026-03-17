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
