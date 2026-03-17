pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let score_threshold = ctx
        .config
        .get("score_threshold")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let metrics_key = ctx.metrics_key;

    let score = ctx.metrics.get_score(metrics_key);

    if score > score_threshold {
        let response = http::Response::builder()
            .status(429)
            .header("Retry-After", "60")
            .header("X-Score", score.to_string())
            .body(bytes::Bytes::new())?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::RateLimit);
