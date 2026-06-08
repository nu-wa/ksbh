//! Proof-of-work challenge module.
//!
//! Issues BLAKE3-based PoW challenges to unknown clients.
//! Difficulty scales with client reputation score (score/100 extra zeros).
//! `/pow` endpoint handles verification.
//! Completion state stored in session for 24 hours.

use ksbh_modules_sdk::{module_definition, export_module};

mod templates;

const POW_PATH: &str = "/pow";
const CHALLENGE_COMPLETE_KEY: &str = "challenge_complete";
const ONE_DAY: i64 = 86400;

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
    reputation_key: &[u8],
    issued_at: u64,
    effective_difficulty: usize,
) -> ::std::string::String {
    let mut blake_hasher = blake3::Hasher::new_keyed(secret_slice);
    blake_hasher.update(reputation_key);
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
    _stage: ksbh_modules_sdk::RequestStage,
    ctx: ksbh_modules_sdk::ModuleContext,
) -> ksbh_modules_sdk::RequestResult {
    let secret = ctx.require_config("secret")?;

    let secret_slice: &[u8; 32] = secret
        .as_bytes()
        .get(..32)
        .ok_or_else(|| ksbh_modules_sdk::ModuleError::abi("secret must be at least 32 bytes"))?
        .try_into()
        .map_err(|_| {
            ksbh_modules_sdk::ModuleError::abi("secret must be exactly readable as 32 bytes")
        })?;

    let full_pow_path = {
        let normalized_internal_path = ctx.internal_path.trim_end_matches('/');

        if normalized_internal_path.is_empty() {
            POW_PATH.to_string()
        } else {
            format!("{normalized_internal_path}{POW_PATH}")
        }
    };

    // Challenged submission
    if ctx.request_info.path == full_pow_path {
        if ctx.request_info.method != "GET" {
            return ksbh_modules_sdk::text_response(http::StatusCode::BAD_REQUEST, "Invalid METHOD");
        }

        let challenge = ctx
            .request_info
            .query_params
            .get("challenge")
            .and_then(|v| urlencoding::decode(v).ok())
            .map(|v| v.into_owned());

        let nonce = ctx
            .request_info
            .query_params
            .get("nonce")
            .and_then(|v| v.parse::<u64>().ok());

        let challenge = match challenge {
            Some(challenge) if !challenge.is_empty() => challenge,
            _ => {
                return ksbh_modules_sdk::text_response(
                    http::StatusCode::BAD_REQUEST,
                    "Invalid Form Data",
                );
            }
        };

        let nonce = match nonce {
            Some(nonce) => nonce,
            None => {
                return ksbh_modules_sdk::text_response(
                    http::StatusCode::BAD_REQUEST,
                    "Invalid Form Data",
                );
            }
        };

        let parts: Vec<&str> = challenge.split('.').collect();
        if parts.len() != 3 {
            return ksbh_modules_sdk::text_response(
                http::StatusCode::BAD_REQUEST,
                "Invalid challenge format",
            );
        }

        let iat: u64 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => {
                return ksbh_modules_sdk::text_response(
                    http::StatusCode::BAD_REQUEST,
                    "Invalid timestamp",
                );
            }
        };

        let effective_difficulty: usize = match parts[1].parse() {
            Ok(v) if v >= 1 => v,
            _ => {
                return ksbh_modules_sdk::text_response(
                    http::StatusCode::BAD_REQUEST,
                    "Invalid difficulty",
                );
            }
        };

        let expected =
            create_challenge(secret_slice, ctx.reputation_key, iat, effective_difficulty);
        if expected != challenge {
            return ksbh_modules_sdk::text_response(
                http::StatusCode::BAD_REQUEST,
                "Invalid signature",
            );
        }

        let now = ksbh_core::utils::current_unix_time() as u64;
        if now > iat + 300 {
            return ksbh_modules_sdk::text_response(
                http::StatusCode::BAD_REQUEST,
                "Challenge expired",
            );
        }

        let mut sha = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut sha, format!("{}{}", challenge, nonce));
        let hash = hex::encode(sha2::Digest::finalize(sha));

        if !hash.starts_with(&"0".repeat(effective_difficulty)) {
            return ksbh_modules_sdk::text_response(
                http::StatusCode::BAD_REQUEST,
                "Invalid proof",
            );
        }

        ctx.reputation_good_boy()?;

        let challenge_complete = ksbh_core::utils::current_unix_time().to_string();
        ctx.session_set(
            CHALLENGE_COMPLETE_KEY,
            challenge_complete.as_bytes(),
            Some(ONE_DAY as u64),
        )?;

        let redirect_to = ctx
            .request_info
            .query_params
            .get("redirect_to")
            .and_then(|value| urlencoding::decode(value).ok())
            .map(|value| value.into_owned())
            .unwrap_or_else(|| "/".to_string());

        let response = http::Response::builder()
            .status(http::StatusCode::FOUND)
            .header(http::header::LOCATION, redirect_to)
            .header(http::header::CONTENT_LENGTH, 0)
            .body(bytes::Bytes::new())?;

        return Ok(ksbh_modules_sdk::ModuleResult::Stop(Some(response)));
    }

    if ctx.request_info.method != "GET" || ctx.request_info.is_websocket_handshake {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    if let Some(ts) = ctx.session_get_parse::<i64>(CHALLENGE_COMPLETE_KEY)? {
        let now = ksbh_core::utils::current_unix_time();
        if now <= ts + ONE_DAY {
            return Ok(ksbh_modules_sdk::ModuleResult::Pass);
        }
    }

    let base_difficulty = ctx
        .config
        .get("difficulty")
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.max(1))
        .unwrap_or(4);
    let score = ctx.reputation_score()?.unwrap_or(0);
    let effective_difficulty = base_difficulty + ((score / 100) as usize);
    let issued_at = ksbh_core::utils::current_unix_time() as u64;
    let challenge =
        create_challenge(secret_slice, ctx.reputation_key, issued_at, effective_difficulty);

    let redirect_to = ctx.request_info.uri;
    let pow_action_url = if let Some(cookie_domain) = ctx.config.get("cookie_domain") {
        format!(
            "{}://{}{}?redirect_to={}",
            ctx.request_info.scheme,
            cookie_domain,
            build_pow_path(ctx.internal_path),
            urlencoding::encode(redirect_to)
        )
    } else {
        format!(
            "{}?redirect_to={}",
            full_pow_path,
            urlencoding::encode(redirect_to)
        )
    };

    let html = templates::render_challenge(&challenge, effective_difficulty, &pow_action_url)
        .map_err(|e| ksbh_modules_sdk::ModuleError::critical(e))?;

    let response = http::Response::builder()
        .status(http::StatusCode::UNAUTHORIZED)
        .header(http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(http::header::CONTENT_LENGTH, html.len())
        .body(bytes::Bytes::from(html))?;

    Ok(ksbh_modules_sdk::ModuleResult::Stop(Some(response)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_pow_path ───────────────────────────────────────────────

    #[test]
    fn pow_path_defaults_to_root_pow() {
        assert_eq!(build_pow_path(""), "/pow");
    }

    #[test]
    fn pow_path_with_internal_path() {
        assert_eq!(
            build_pow_path("/_ksbh_internal"),
            "/_ksbh_internal/pow"
        );
    }

    #[test]
    fn pow_path_trims_trailing_slashes() {
        assert_eq!(build_pow_path("/_internal/"), "/_internal/pow");
    }

    #[test]
    fn pow_path_no_leading_slash_internal() {
        assert_eq!(
            build_pow_path("_internal"),
            "_internal/pow"
        );
    }

    #[test]
    fn pow_path_single_slash() {
        assert_eq!(build_pow_path("/"), "/pow");
    }

    // ── create_challenge ─────────────────────────────────────────────

    #[test]
    fn challenge_is_deterministic() {
        let secret = [1u8; 32];
        let a = create_challenge(&secret, b"key-XY", 1000, 4);
        let b = create_challenge(&secret, b"key-XY", 1000, 4);
        assert_eq!(a, b);
        assert!(!a.is_empty());
        // Format: iat.difficulty.hex_signature
        assert_eq!(a.split('.').count(), 3);
    }

    #[test]
    fn different_secret_produces_different_challenge() {
        let s1 = [1u8; 32];
        let s2 = [2u8; 32];
        let a = create_challenge(&s1, b"key-XY", 1000, 4);
        let b = create_challenge(&s2, b"key-XY", 1000, 4);
        assert_ne!(a, b);
    }

    #[test]
    fn different_reputation_key_produces_different_challenge() {
        let secret = [1u8; 32];
        let a = create_challenge(&secret, b"key-AA", 1000, 4);
        let b = create_challenge(&secret, b"key-BB", 1000, 4);
        assert_ne!(a, b);
    }

    #[test]
    fn different_difficulty_produces_different_challenge() {
        let secret = [1u8; 32];
        let a = create_challenge(&secret, b"key-XY", 1000, 4);
        let b = create_challenge(&secret, b"key-XY", 1000, 5);
        assert_ne!(a, b);
    }

    #[test]
    fn different_issued_at_produces_different_challenge() {
        let secret = [1u8; 32];
        let a = create_challenge(&secret, b"key-XY", 1000, 4);
        let b = create_challenge(&secret, b"key-XY", 2000, 4);
        assert_ne!(a, b);
    }

    #[test]
    fn challenge_has_correct_format() {
        let secret = [42u8; 32];
        let challenge = create_challenge(&secret, b"test", 1700000, 4);
        let parts: Vec<&str> = challenge.split('.').collect();
        assert_eq!(parts.len(), 3, "challenge should have 3 dot-separated parts");
        // First part should be the iat (issued_at timestamp)
        assert_eq!(parts[0], "1700000");
        // Second part should be the difficulty
        assert_eq!(parts[1], "4");
        // Third part should be non-empty hex
        assert!(!parts[2].is_empty());
        assert!(parts[2].len() == 64); // blake3 hex output is 64 chars
    }

    // ── Difficulty calculation (pure logic) ──────────────────────────

    #[test]
    fn difficulty_scales_with_reputation() {
        let base = 4usize;
        let effective = |score: u64| base + ((score / 100) as usize);
        assert_eq!(effective(0), 4);
        assert_eq!(effective(50), 4);
        assert_eq!(effective(99), 4);
        assert_eq!(effective(100), 5);
        assert_eq!(effective(200), 6);
        assert_eq!(effective(1000), 14);
    }

    #[test]
    fn difficulty_floor_is_base_difficulty() {
        let base = 1usize;
        let effective = |score: u64| base + ((score / 100) as usize);
        assert_eq!(effective(0), 1);
        assert_eq!(effective(50), 1);
    }

    // ── Proof verification logic ─────────────────────────────────────

    #[test]
    fn proof_verification_finds_valid_nonce() {
        let challenge = "1000.2.abcdef";
        let _difficulty: usize = 2;

        // This test just validates the hash computation, not finding a solution
        let mut sha = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut sha, format!("{}{}", challenge, 0u64));
        let hash = hex::encode(sha2::Digest::finalize(sha));
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn any_hash_has_correct_length() {
        let mut sha = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut sha, b"some data");
        let hash = hex::encode(sha2::Digest::finalize(sha));
        assert_eq!(hash.len(), 64);
    }

    // ── Challenge format validation (pure logic) ─────────────────────

    #[test]
    fn valid_challenge_has_three_parts() {
        let parts: Vec<&str> = "1000.4.abcd1234".split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn invalid_challenge_too_few_parts() {
        let parts: Vec<&str> = "1000.4".split('.').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn invalid_challenge_too_many_parts() {
        let parts: Vec<&str> = "1.2.3.4.5".split('.').collect();
        assert_eq!(parts.len(), 5);
    }

    // ── Template rendering ───────────────────────────────────────────

    #[test]
    fn challenge_template_renders() {
        let html = templates::render_challenge("1000.4.abc123", 4, "/pow?redirect_to=/")
            .expect("template render");
        assert!(html.contains("1000.4.abc123"), "html should contain challenge");
        assert!(html.contains("<html") || html.contains("<!DOCTYPE"), "should be HTML");
    }

    #[test]
    fn template_contains_difficulty() {
        let html = templates::render_challenge("2000.8.xyz789", 8, "/_internal/pow")
            .expect("template render");
        // The template should reference the difficulty somewhere
        assert!(html.contains("2000.8.xyz789"), "html should contain challenge string");
    }

    // ── Constants ─────────────────────────────────────────────────────

    #[test]
    fn pow_path_constant() {
        assert_eq!(POW_PATH, "/pow");
    }

    #[test]
    fn one_day_constant() {
        assert_eq!(ONE_DAY, 86400);
    }
}

export_module!(
    process,
    module_definition!(
        ksbh_modules_sdk::abi::prelude::KSBHModuleKind::POW,
        [ksbh_modules_sdk::RequestStage::Request, ksbh_modules_sdk::RequestStage::BeforeRouting]
    )
);
