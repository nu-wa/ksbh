mod templates;

const POW_PATH: &str = "/pow";
const CHALLENGE_COMPLETE_KEY: &str = "challenge_complete";
const ONE_DAY: i64 = 86400;

pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let path = ctx.request.path.as_str();
    let internal_path = ctx.internal_path;
    let full_pow_path = format!("{}{}", internal_path, POW_PATH);

    if path == full_pow_path {
        return handle_pow_verification(ctx);
    }

    let difficulty = ctx
        .config
        .get("difficulty")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    let secret = ctx
        .config
        .get("secret")
        .map_or("tell nabil from morrocco to change the secret", |v| v)
        .to_string();

    let secret_slice: &[u8; 32] = match secret.as_bytes()[0..32].try_into() {
        Ok(slice) => slice,
        Err(_) => {
            ksbh_modules_sdk::log_error!(ctx.logger, "Secret must be at least 32 bytes");
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    };

    let metrics_key = ctx.metrics_key.to_vec();

    let score = ctx.metrics.get_score(&metrics_key);

    let mut actual_difficulty = difficulty;
    actual_difficulty += (score / 100) as usize;

    let now = ksbh_core::utils::current_unix_time();
    let mut challenge_complete: Option<i64> = None;
    let mut challenge_expired = false;

    if let Some(stored_challenge_complete) = ctx.session.get(CHALLENGE_COMPLETE_KEY) {
        match ::std::str::from_utf8(&stored_challenge_complete)
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
        {
            Some(ts) => {
                if now > ts + ONE_DAY {
                    challenge_expired = true;
                } else {
                    challenge_complete = Some(ts);
                }
            }
            None => {
                ksbh_modules_sdk::log_debug!(
                    ctx.logger,
                    "Failed to parse challenge_complete session value"
                );
            }
        }
    }

    if challenge_complete.is_some() && !challenge_expired {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    if ctx.request.method.as_str() != "GET" {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let iat = ksbh_core::utils::current_unix_time() as u64;

    let mut b3_hasher = blake3::Hasher::new_keyed(secret_slice);
    b3_hasher.update(&metrics_key);
    b3_hasher.update(iat.to_string().as_bytes());

    let signature = b3_hasher.finalize();
    let challenge = format!("{}.{}", iat, signature.to_hex());

    let redirect_to = ctx.request.uri.as_str();

    let pow_action_url = if let Some(cookie_domain) = ctx.config.get("cookie_domain") {
        let scheme = ctx.request.scheme.as_str();
        let internal_path = ctx.internal_path;
        format!(
            "{}://{}{}{}?redirect_to={}",
            scheme,
            cookie_domain,
            internal_path,
            POW_PATH,
            urlencoding::encode(redirect_to).into_owned()
        )
    } else {
        let full_pow_path = format!("{}{}", internal_path, POW_PATH);
        format!(
            "{}?redirect_to={}",
            full_pow_path,
            urlencoding::encode(redirect_to).into_owned()
        )
    };

    let html = match templates::render_challenge(
        &challenge,
        actual_difficulty,
        &pow_action_url,
        redirect_to,
    ) {
        Ok(html) => html,
        Err(e) => {
            ksbh_modules_sdk::log_error!(ctx.logger, "Failed to render template: {}", e);
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    };

    let response = match http::Response::builder()
        .status(http::StatusCode::UNAUTHORIZED)
        .header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(http::header::CONTENT_LENGTH, html.len())
        .body(bytes::Bytes::from(html))
    {
        Ok(r) => r,
        Err(e) => {
            ksbh_modules_sdk::log_error!(ctx.logger, "Failed to build response: {}", e);
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    };

    Ok(ksbh_modules_sdk::ModuleResult::Stop(response))
}

fn handle_pow_verification(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    if ctx.request.method.as_str() != "POST" {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid METHOD"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let difficulty = ctx
        .config
        .get("difficulty")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    let secret = ctx
        .config
        .get("secret")
        .map_or("tell nabil from morrocco to change the secret", |v| v)
        .to_string();

    let secret_slice: &[u8; 32] = match secret.as_bytes()[0..32].try_into() {
        Ok(slice) => slice,
        Err(_) => {
            ksbh_modules_sdk::log_error!(ctx.logger, "Secret must be at least 32 bytes");
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    };

    let metrics_key = ctx.metrics_key;

    let body_str = match ::std::str::from_utf8(ctx.body) {
        Ok(s) => s,
        Err(_) => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid body"))?;
            return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
        }
    };

    let mut challenge = String::new();
    let mut nonce: u64 = 0;

    for part in body_str.split('&') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let val = kv.next().unwrap_or("");

        let decoded_val = match urlencoding::decode(val) {
            Ok(s) => s.into_owned(),
            Err(_) => continue,
        };

        match key {
            "challenge" => challenge = decoded_val,
            "nonce" => nonce = decoded_val.parse().unwrap_or(0),
            _ => {}
        }
    }

    if challenge.is_empty() || nonce == 0 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid Form Data"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let parts: Vec<&str> = challenge.split('.').collect();
    if parts.len() != 2 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid challenge format"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let (iat_str, user_submitted_signature) = (parts[0], parts[1]);

    let mut b3_hasher = blake3::Hasher::new_keyed(secret_slice);
    b3_hasher.update(metrics_key);
    b3_hasher.update(iat_str.as_bytes());

    let real_signature = b3_hasher.finalize();
    if real_signature.to_hex().as_str() != user_submitted_signature {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid signature"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let iat: u64 = match iat_str.parse() {
        Ok(v) => v,
        Err(_) => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid timestamp"))?;
            return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
        }
    };

    let now = ksbh_core::utils::current_unix_time() as u64;

    if now > iat + 300 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Challenge expired"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    use sha2::Digest;
    let mut sha = sha2::Sha256::new();
    sha.update(format!("{}{}", challenge, nonce));
    let hash = hex::encode(sha.finalize());

    if !hash.starts_with(&"0".repeat(difficulty)) {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid proof"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    ctx.metrics.good_boy(metrics_key);

    let redirect_to = ctx
        .request
        .query_params
        .get("redirect_to")
        .map(|s| s.as_str())
        .unwrap_or("/");

    let challenge_complete = ksbh_core::utils::current_unix_time().to_string();

    if !ctx.session.set_with_ttl(
        CHALLENGE_COMPLETE_KEY,
        challenge_complete.as_bytes(),
        ONE_DAY as u64,
    ) {
        ksbh_modules_sdk::log_error!(
            ctx.logger,
            "Failed to persist challenge_complete in session storage"
        );
        let response = http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(bytes::Bytes::from_static(b"Internal error"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let response = http::Response::builder()
        .status(http::StatusCode::FOUND)
        .header(http::header::LOCATION, redirect_to)
        .header(http::header::CONTENT_LENGTH, 0)
        .body(bytes::Bytes::new())?;

    Ok(ksbh_modules_sdk::ModuleResult::Stop(response))
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::Pow);
