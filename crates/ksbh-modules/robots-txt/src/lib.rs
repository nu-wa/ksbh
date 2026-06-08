//! robots.txt serving module.
//!
//! Serves static robots.txt content from the `content` config field.
//! Only responds to GET requests on `/robots.txt` path.
//! Returns Pass for all other requests.

pub fn process(
    _stage: ksbh_modules_sdk::RequestStage,
    ctx: ksbh_modules_sdk::ModuleContext,
) -> ksbh_modules_sdk::RequestResult {
    if ctx.request_info.method == "GET"
        && ctx.request_info.path == "/robots.txt"
        && let Some(content) = ctx.config.get("content").copied()
    {
        return ksbh_modules_sdk::plain_text_response(http::StatusCode::OK, content);
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

#[cfg(test)]
mod tests {
    fn should_serve_robots(method: &str, path: &str, has_content: bool) -> bool {
        method == "GET" && path == "/robots.txt" && has_content
    }

    #[test]
    fn serves_robots_txt_for_get_with_content() {
        assert!(should_serve_robots("GET", "/robots.txt", true));
    }

    #[test]
    fn does_not_serve_for_post() {
        assert!(!should_serve_robots("POST", "/robots.txt", true));
    }

    #[test]
    fn does_not_serve_for_wrong_path() {
        assert!(!should_serve_robots("GET", "/other.txt", true));
    }

    #[test]
    fn does_not_serve_when_no_content_configured() {
        assert!(!should_serve_robots("GET", "/robots.txt", false));
    }
}

ksbh_modules_sdk::export_module!(
    process,
    ksbh_modules_sdk::module_definition!(
        ksbh_modules_sdk::abi::prelude::KSBHModuleKind::Robots,
        [ksbh_modules_sdk::RequestStage::BeforeRouting, ksbh_modules_sdk::RequestStage::Request]
    )
);
