pub fn process(ctx: ksbh_modules_sdk::RequestContext) -> ksbh_modules_sdk::ModuleResult {
    let scheme = ctx.request.scheme.as_str();
    let port = ctx.request.port;
    let uri = ctx.request.uri.as_str();
    let secure = scheme == "https"
        || port == 443
        || uri.starts_with("https://")
        || uri.starts_with("wss://");

    if !secure {
        let redirect_url = if uri.starts_with("http://") {
            uri.replacen("http://", "https://", 1)
        } else {
            format!("https://{}", uri)
        };

        let response = http::Response::builder()
            .status(http::StatusCode::MOVED_PERMANENTLY)
            .header(http::header::LOCATION, redirect_url)
            .body(bytes::Bytes::new())
            .unwrap();

        return ksbh_modules_sdk::ModuleResult::Stop(response);
    }

    ksbh_modules_sdk::ModuleResult::Pass
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::HttpToHttps);
