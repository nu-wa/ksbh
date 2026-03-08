use ksbh_modules_sdk::{ModuleResult, RequestContext};

pub fn process(_ctx: RequestContext) -> ModuleResult {
    ModuleResult::Pass
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::RateLimit);
