pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let previous_seen = ctx
        .session
        .get("seen")
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default();

    let request_body = String::from_utf8_lossy(ctx.body).to_string();
    let current_value = format!("{}|{}", ctx.request.path, request_body);

    let session_set = ctx.session.set("seen", current_value.as_bytes());
    let session_set_ttl = ctx
        .session
        .set_with_ttl("seen_ttl", request_body.as_bytes(), 60);

    if !session_set || !session_set_ttl {
        return Err(ksbh_modules_sdk::ModuleError::internal_error(
            "failed to persist session state",
        ));
    }

    let score_before = ctx.metrics.get_score(ctx.metrics_key);
    let good_boy = ctx.metrics.good_boy(ctx.metrics_key);
    let score_after = ctx.metrics.get_score(ctx.metrics_key);

    let response_body = format!(
        "path={};method={};body={};seen_before={};score_before={};score_after={};good_boy={}",
        ctx.request.path,
        ctx.request.method,
        request_body,
        previous_seen,
        score_before,
        score_after,
        good_boy
    );

    let response = http::Response::builder()
        .status(http::StatusCode::OK)
        .header("content-type", "text/plain")
        .header("x-module-name", ctx.mod_name.as_str())
        .header("x-seen-before", previous_seen)
        .header("x-score-before", score_before.to_string())
        .header("x-score-after", score_after.to_string())
        .header("x-good-boy", good_boy.to_string())
        .header("x-internal-path", ctx.internal_path)
        .body(bytes::Bytes::from(response_body))?;

    Ok(ksbh_modules_sdk::ModuleResult::Stop(response))
}

ksbh_modules_sdk::register_module!(
    process,
    ksbh_modules_sdk::types::ModuleType::Custom("dynamic-ffi-smoke".into())
);
