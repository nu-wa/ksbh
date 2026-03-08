use ksbh_modules_sdk::{ModuleResult, RequestContext};

mod templates;

const POW_PATH: &str = "/pow";

pub fn process(ctx: RequestContext) -> ModuleResult {
    let path = ctx.request.path.as_str();

    let difficulty = ctx
        .config
        .get("difficulty")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4);

    let secret = ctx
        .config
        .get("secret")
        .map_or("tell nabil from morrocco to change the secret", |v| v);

    let secret_slice: &[u8; 32] = match secret.as_bytes()[0..32].try_into() {
        Ok(slice) => slice,
        Err(_) => {
            ksbh_modules_sdk::log_error!(ctx.logger, "Secret must be at least 32 bytes");
            return ModuleResult::Pass;
        }
    };

    let client_ip = ctx.client_ip.clone();
    let client_ip_str = client_ip.as_str();
    let user_agent_clone = ctx.user_agent.clone();
    let user_agent = user_agent_clone.as_ref().map(|s| s.as_str());

    let (hits_good, hits_bad) = ctx
        .metrics
        .get_hits(client_ip_str, user_agent)
        .map(|(g, b)| (g as usize, b as usize))
        .unwrap_or((1, 0));

    let mut actual_difficulty = difficulty;
    if hits_bad >= hits_good {
        actual_difficulty += hits_bad / 10;
    }

    let now = chrono::Utc::now().naive_utc();
    static ONE_DAY: ::std::time::Duration = ::std::time::Duration::from_hours(24);

    let cookie_header = ctx.cookie_header.as_str();
    let mut challenge_complete: Option<chrono::NaiveDateTime> = None;
    let mut challenge_expired = false;

    if !cookie_header.is_empty() {
        match ksbh_core::cookies::ProxyCookie::from_cookie_header(cookie_header) {
            Ok(cookie) => {
                if let Some(ts) = cookie.challenge_complete {
                    if now > ts + chrono::Duration::from_std(ONE_DAY).unwrap() {
                        challenge_expired = true;
                    } else {
                        challenge_complete = Some(ts);
                    }
                }
            }
            Err(e) => {
                ksbh_modules_sdk::log_debug!(ctx.logger, "Failed to parse cookie: {}", e);
            }
        }
    }

    if path.starts_with(POW_PATH) {
        return handle_pow_verification(
            ctx,
            secret_slice,
            actual_difficulty,
            client_ip_str,
            user_agent,
        );
    }

    if challenge_complete.is_some() && !challenge_expired {
        return ModuleResult::Pass;
    }

    if ctx.request.method.as_str() != "GET" {
        return ModuleResult::Pass;
    }

    let iat = ::std::time::SystemTime::now()
        .duration_since(::std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut b3_hasher = blake3::Hasher::new_keyed(secret_slice);
    b3_hasher.update(client_ip_str.as_bytes());
    if let Some(ua) = user_agent {
        b3_hasher.update(ua.as_bytes());
    }
    b3_hasher.update(iat.to_string().as_bytes());

    let signature = b3_hasher.finalize();
    let challenge = format!("{}.{}", iat, signature.to_hex());

    let redirect_to = ctx.request.uri.as_str();

    let html =
        match templates::render_challenge(&challenge, actual_difficulty, POW_PATH, redirect_to) {
            Ok(html) => html,
            Err(e) => {
                ksbh_modules_sdk::log_error!(ctx.logger, "Failed to render template: {}", e);
                return ModuleResult::Pass;
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
            return ModuleResult::Pass;
        }
    };

    ModuleResult::Stop(response)
}

fn handle_pow_verification(
    ctx: RequestContext,
    secret_slice: &[u8; 32],
    difficulty: usize,
    client_ip: &str,
    user_agent: Option<&str>,
) -> ModuleResult {
    let client_ip_owned = client_ip.to_string();
    if ctx.request.method.as_str() != "POST" {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid METHOD"))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    let body_str = match ::std::str::from_utf8(ctx.body) {
        Ok(s) => s,
        Err(_) => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid body"))
                .unwrap();
            return ModuleResult::Stop(response);
        }
    };

    let mut challenge = String::new();
    let mut nonce: u64 = 0;
    let mut redirect_to = String::from("/");

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
            "redirect_to" => redirect_to = decoded_val,
            _ => {}
        }
    }

    if challenge.is_empty() || nonce == 0 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid Form Data"))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    let parts: Vec<&str> = challenge.split('.').collect();
    if parts.len() != 2 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid challenge format"))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    let (iat_str, user_submitted_signature) = (parts[0], parts[1]);

    let mut b3_hasher = blake3::Hasher::new_keyed(secret_slice);
    b3_hasher.update(client_ip_owned.as_bytes());
    if let Some(ua) = user_agent {
        b3_hasher.update(ua.as_bytes());
    }
    b3_hasher.update(iat_str.as_bytes());

    let real_signature = b3_hasher.finalize();
    if real_signature.to_hex().as_str() != user_submitted_signature {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid signature"))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    let iat: u64 = match iat_str.parse() {
        Ok(v) => v,
        Err(_) => {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(bytes::Bytes::from_static(b"Invalid timestamp"))
                .unwrap();
            return ModuleResult::Stop(response);
        }
    };

    let now = ::std::time::SystemTime::now()
        .duration_since(::std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if now > iat + 300 {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Challenge expired"))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    use sha2::Digest;
    let mut sha = sha2::Sha256::new();
    sha.update(format!("{}{}", challenge, nonce));
    let hash = hex::encode(sha.finalize());

    if !hash.starts_with(&"0".repeat(difficulty)) {
        let response = http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .body(bytes::Bytes::from_static(b"Invalid proof"))
            .unwrap();
        return ModuleResult::Stop(response);
    }

    ctx.metrics.increment_good(client_ip, user_agent);

    let cookie_header = ctx.cookie_header.as_str();
    let mut proxy_cookie = if !cookie_header.is_empty() {
        match ksbh_core::cookies::ProxyCookie::from_cookie_header(cookie_header) {
            Ok(c) => c,
            Err(_) => ksbh_core::cookies::ProxyCookie::new(
                ctx.request.host.as_str(),
                None,
                uuid::Uuid::new_v4(),
            ),
        }
    } else {
        ksbh_core::cookies::ProxyCookie::new(ctx.request.host.as_str(), None, uuid::Uuid::new_v4())
    };

    proxy_cookie.challenge_complete = Some(chrono::Utc::now().naive_utc());

    let cookie_value = match proxy_cookie.to_cookie_header() {
        Ok(v) => v,
        Err(e) => {
            ksbh_modules_sdk::log_error!(ctx.logger, "Failed to serialize cookie: {}", e);
            let response = http::Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(bytes::Bytes::from_static(b"Internal error"))
                .unwrap();
            return ModuleResult::Stop(response);
        }
    };

    let response = http::Response::builder()
        .status(http::StatusCode::FOUND)
        .header(http::header::LOCATION, redirect_to)
        .header(http::header::SET_COOKIE, cookie_value)
        .header(http::header::CONTENT_LENGTH, 0)
        .body(bytes::Bytes::new())
        .unwrap();

    ModuleResult::Stop(response)
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::Pow);
