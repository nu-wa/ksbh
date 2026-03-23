//! Proof-of-work challenge module.
//!
//! Issues BLAKE3-based PoW challenges to unknown clients.
//! Difficulty scales with client score (score/100 extra zeros).
//! `/pow` endpoint handles verification.
//! Completion state stored in session for 24 hours.

mod templates;

const POW_PATH: &str = "/pow";
const CHALLENGE_COMPLETE_KEY: &str = "challenge_complete";
const ONE_DAY: i64 = 86400;

fn parse_base_difficulty(configured_value: Option<&str>) -> usize {
    configured_value
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value: usize| value.max(1))
        .unwrap_or(4)
}

fn compute_effective_difficulty(base_difficulty: usize, score: u64) -> usize {
    base_difficulty + ((score / 100) as usize)
}

fn build_pow_path(internal_path: &str) -> ::std::string::String {
    let normalized_internal_path = internal_path.trim_end_matches('/');

    if normalized_internal_path.is_empty() {
        POW_PATH.to_string()
    } else {
        format!("{normalized_internal_path}{POW_PATH}")
    }
}

fn create_challenge(
    secret_slice: &[u8; 32],
    metrics_key: &[u8],
    issued_at: u64,
    effective_difficulty: usize,
) -> ::std::string::String {
    let mut blake_hasher = blake3::Hasher::new_keyed(secret_slice);
    blake_hasher.update(metrics_key);
    blake_hasher.update(issued_at.to_string().as_bytes());
    blake_hasher.update(effective_difficulty.to_string().as_bytes());

    let signature = blake_hasher.finalize();
    format!(
        "{}.{}.{}",
        issued_at,
        effective_difficulty,
        signature.to_hex()
    )
}

pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let path = ctx.request.path.as_str();
    let internal_path = ctx.internal_path;
    let full_pow_path = build_pow_path(internal_path);

    if path == full_pow_path {
        return handle_pow_verification(ctx);
    }

    let difficulty =
        parse_base_difficulty(ctx.config.get("difficulty").map(|value| value.as_str()));

    let secret = ctx
        .config
        .get("secret")
        .map_or("tell nabil from morrocco to change the secret", |v| v)
        .to_string();

    let secret_slice: &[u8; 32] = match secret.as_bytes()[0..32].try_into() {
        Ok(slice) => slice,
        Err(_) => {
            ksbh_modules_sdk::log_warn!(ctx.logger, "Secret must be at least 32 bytes");
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    };

    let metrics_key = ctx.metrics_key.to_vec();

    let score = ctx.metrics.get_score(&metrics_key);

    let actual_difficulty = compute_effective_difficulty(difficulty, score);

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

    let challenge = create_challenge(secret_slice, &metrics_key, iat, actual_difficulty);

    let redirect_to = ctx.request.uri.as_str();

    let pow_action_url = if let Some(cookie_domain) = ctx.config.get("cookie_domain") {
        let scheme = ctx.request.scheme.as_str();
        format!(
            "{}://{}{}?redirect_to={}",
            scheme,
            cookie_domain,
            build_pow_path(internal_path),
            urlencoding::encode(redirect_to).into_owned()
        )
    } else {
        format!(
            "{}?redirect_to={}",
            full_pow_path,
            urlencoding::encode(redirect_to).into_owned()
        )
    };

    let html = match templates::render_challenge(&challenge, actual_difficulty, &pow_action_url) {
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

    let secret = ctx
        .config
        .get("secret")
        .map_or("tell nabil from morrocco to change the secret", |v| v)
        .to_string();

    let secret_slice: &[u8; 32] = match secret.as_bytes()[0..32].try_into() {
        Ok(slice) => slice,
        Err(_) => {
            ksbh_modules_sdk::log_warn!(ctx.logger, "Secret must be at least 32 bytes");
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    };

    let metrics_key = ctx.metrics_key;

    ksbh_modules_sdk::log_debug!(
        ctx.logger,
        "PoW verification request body len={}, path={}, query_params={:?}",
        ctx.body.len(),
        ctx.request.path,
        ctx.request.query_params
    );

    let body_str = match ::std::str::from_utf8(ctx.body) {
        Ok(s) => s,
        Err(_) => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid body"))?;
            return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
        }
    };

    ksbh_modules_sdk::log_debug!(ctx.logger, "PoW verification raw body=`{}`", body_str);

    let mut challenge = String::new();
    let mut nonce: Option<u64> = None;

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
            "nonce" => nonce = decoded_val.parse().ok(),
            _ => {}
        }
    }

    ksbh_modules_sdk::log_debug!(
        ctx.logger,
        "PoW verification parsed challenge_present={}, nonce={:?}",
        !challenge.is_empty(),
        nonce
    );

    if challenge.is_empty() || nonce.is_none() {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid Form Data"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let parts: Vec<&str> = challenge.split('.').collect();
    if parts.len() != 3 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid challenge format"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let (iat_str, effective_difficulty_str, _) = (parts[0], parts[1], parts[2]);

    let effective_difficulty: usize = match effective_difficulty_str.parse() {
        Ok(value) if value >= 1 => value,
        _ => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid difficulty"))?;
            return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
        }
    };

    let iat: u64 = match iat_str.parse() {
        Ok(v) => v,
        Err(_) => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid timestamp"))?;
            return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
        }
    };

    let expected_challenge = create_challenge(secret_slice, metrics_key, iat, effective_difficulty);
    if expected_challenge != challenge {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid signature"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let now = ksbh_core::utils::current_unix_time() as u64;

    if now > iat + 300 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Challenge expired"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    let mut sha = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(
        &mut sha,
        format!("{}{}", challenge, nonce.unwrap_or_default()),
    );
    let hash = hex::encode(sha2::Digest::finalize(sha));

    if !hash.starts_with(&"0".repeat(effective_difficulty)) {
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
        .and_then(|value| urlencoding::decode(value).ok())
        .map(|value| value.into_owned())
        .unwrap_or_else(|| "/".to_string());

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
