use ksbh_modules_sdk::{ModuleResult, RequestContext};

pub fn process(ctx: RequestContext) -> ModuleResult {
    let path = ctx.request.path.as_str();
    let method = ctx.request.method.as_str();

    if method == "GET" && path == "/robots.txt" {
        let content = match ctx.config.get("content") {
            Some(c) => c.to_string(),
            None => {
                return ModuleResult::Pass;
            }
        };

        let response = http::Response::builder()
            .status(http::StatusCode::OK)
            .header("Content-Type", "text/plain")
            .body(bytes::Bytes::from(content))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    ModuleResult::Pass
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::RobotsTxt);
