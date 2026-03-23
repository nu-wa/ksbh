//! robots.txt serving module.
//!
//! Serves static robots.txt content from the `content` config field.
//! Only responds to GET requests on `/robots.txt` path.
//! Returns Pass for all other requests.

pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let path = ctx.request.path.as_str();
    let method = ctx.request.method.as_str();

    if method == "GET" && path == "/robots.txt" {
        let content = match ctx.config.get("content") {
            Some(c) => c.to_string(),
            None => {
                return Ok(ksbh_modules_sdk::ModuleResult::Pass);
            }
        };

        let response = http::Response::builder()
            .status(http::StatusCode::OK)
            .header("Content-Type", "text/plain")
            .body(bytes::Bytes::from(content))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::RobotsTxt);
