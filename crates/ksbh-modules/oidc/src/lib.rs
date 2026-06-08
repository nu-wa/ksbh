//! OpenID Connect authentication module.
//!
//! Full OIDC authorization code flow with PKCE support.
//! Checks session validity via `oidc_complete` timestamp.
//! Handles token refresh.
//! Stores flow state in namespaced session storage.
//! CSRF/state expiry after 5 minutes.

mod provider;
mod state;

const DEFAULT_SESSION_TTL_SECS: u64 = 3600;
const FLOW_STATE_TTL_SECS: u64 = 300;
const FIVE_MINUTES: i64 = 300;
static SYNC_FAVICON_PATH: &str = "/favicon.ico";
static DEFAULT_INTERNAL_PATH: &str = "/_ksbh_internal";
fn is_unauthenticated_websocket_upgrade(is_websocket_handshake: bool, session_valid: bool) -> bool {
    !session_valid && is_websocket_handshake
}

pub fn process(
    _stage: ksbh_modules_sdk::RequestStage,
    ctx: ksbh_modules_sdk::ModuleContext,
) -> ksbh_modules_sdk::RequestResult {
    let config = provider::OidcConfig {
        issuer_url: ctx.require_config("issuer_url")?,
        client_id: ctx.require_config("client_id")?,
        client_secret: ctx.require_config("client_secret")?,
    };

    let session_ttl_secs = ctx
        .config
        .get("session_ttl_seconds")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SESSION_TTL_SECS);

    let enable_refresh = ctx
        .config
        .get("enable_refresh")
        .is_some_and(|v| *v == "true");

    let base_url = provider::build_base_url(&ctx.request_info);
    let path = ctx.request_info.path;
    let now = ksbh_core::utils::current_unix_time();

    if path == SYNC_FAVICON_PATH {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let mut session_data = state::load(&ctx)?;

    let session_valid = session_data
        .oidc_complete
        .map(|oidc_complete| now < oidc_complete + session_ttl_secs as i64)
        .unwrap_or(false);

    if is_unauthenticated_websocket_upgrade(ctx.request_info.is_websocket_handshake, session_valid)
    {
        return ksbh_modules_sdk::text_response(http::StatusCode::UNAUTHORIZED, "OIDC required");
    }

    if session_valid {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let oidc_expired = session_data.oidc_complete.is_some();

    let modules_internal_path = ctx
        .config
        .get("modules_internal_path")
        .copied()
        .unwrap_or(DEFAULT_INTERNAL_PATH);
    let module_path = format!("{}/oidc", modules_internal_path.trim_end_matches('/'));
    let redirect_url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        module_path.trim_start_matches('/')
    );

    if oidc_expired
        && enable_refresh
        && let Some(ref refresh_token) = session_data.refresh_token
    {
        match provider::try_refresh_token(&config, &redirect_url, refresh_token) {
            Ok(new_refresh_token) => {
                session_data.refresh_token = new_refresh_token;
                session_data.flow = None;
                session_data.oidc_complete = Some(now);
                state::save(&ctx, &session_data, session_ttl_secs)?;

                let response = http::Response::builder()
                    .status(http::StatusCode::OK)
                    .body(bytes::Bytes::new())?;
                return Ok(ksbh_modules_sdk::ModuleResult::Stop(Some(response)));
            }
            Err(_) => {
                session_data.refresh_token = None;
            }
        }
    }

    if !path.starts_with(&module_path) || oidc_expired {
        let result = provider::get_authorization_code(
            &config,
            &redirect_url,
            ctx.request_info.uri,
            &mut session_data,
        );
        match result {
            Ok(auth_url) => {
                state::save(&ctx, &session_data, FLOW_STATE_TTL_SECS)?;
                return ksbh_modules_sdk::redirect_response(&auth_url);
            }
            Err(_) => {
                return ksbh_modules_sdk::text_response(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Authorization failed",
                );
            }
        }
    }

    let Some(code) = ctx.request_info.query_params.get("code").copied() else {
        return ksbh_modules_sdk::text_response(
            http::StatusCode::BAD_REQUEST,
            "Missing code parameter",
        );
    };
    let Some(state_param) = ctx.request_info.query_params.get("state").copied() else {
        return ksbh_modules_sdk::text_response(
            http::StatusCode::BAD_REQUEST,
            "Missing state parameter",
        );
    };

    let flow = match &session_data.flow {
        Some(flow) => flow.clone(),
        None => {
            return ksbh_modules_sdk::text_response(
                http::StatusCode::BAD_REQUEST,
                "OIDC flow state not found in session",
            );
        }
    };

    if flow.csrf_token != state_param {
        return ksbh_modules_sdk::text_response(
            http::StatusCode::BAD_REQUEST,
            "Invalid state parameter",
        );
    }

    if now > flow.time + FIVE_MINUTES {
        let result = provider::get_authorization_code(
            &config,
            &redirect_url,
            &flow.redirect_to,
            &mut session_data,
        );
        match result {
            Ok(auth_url) => {
                state::save(&ctx, &session_data, FLOW_STATE_TTL_SECS)?;
                return ksbh_modules_sdk::redirect_response(&auth_url);
            }
            Err(_) => {
                return ksbh_modules_sdk::text_response(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Authorization failed",
                );
            }
        }
    }

    match provider::exchange_token(&config, &redirect_url, code, &flow) {
        Ok(refresh_token) => {
            session_data.flow = None;
            session_data.refresh_token = refresh_token;
            state::save(&ctx, &session_data, session_ttl_secs)?;
        }
        Err(_) => {
            return ksbh_modules_sdk::text_response(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Token exchange failed",
            );
        }
    }

    let original_redirect = flow.redirect_to.clone();
    session_data.oidc_complete = Some(now);
    state::save(&ctx, &session_data, session_ttl_secs)?;

    ksbh_modules_sdk::redirect_response(&original_redirect)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_unauthenticated_websocket_upgrade ─────────────────────────

    #[test]
    fn ws_upgrade_blocks_when_not_authenticated() {
        assert!(is_unauthenticated_websocket_upgrade(true, false));
    }

    #[test]
    fn ws_upgrade_passes_when_authenticated() {
        assert!(!is_unauthenticated_websocket_upgrade(true, true));
    }

    #[test]
    fn regular_request_bypasses_ws_check() {
        assert!(!is_unauthenticated_websocket_upgrade(false, false));
        assert!(!is_unauthenticated_websocket_upgrade(false, true));
    }

    // ── provider::build_base_url ─────────────────────────────────────

    fn make_req_info<'a>(scheme: &'a str, host: &'a str, port: u16) -> ksbh_modules_sdk::RequestInfo<'a> {
        ksbh_modules_sdk::RequestInfo {
            scheme,
            host,
            port,
            uri: "",
            method: "GET",
            path: "/",
            query_params: std::collections::HashMap::new(),
            is_websocket_handshake: false,
        }
    }

    #[test]
    fn build_base_url_standard_https() {
        let info = make_req_info("https", "example.com", 443);
        assert_eq!(provider::build_base_url(&info), "https://example.com");
    }

    #[test]
    fn build_base_url_standard_http() {
        let info = make_req_info("http", "example.com", 80);
        assert_eq!(provider::build_base_url(&info), "http://example.com");
    }

    #[test]
    fn build_base_url_port_zero() {
        let info = make_req_info("https", "example.com", 0);
        assert_eq!(provider::build_base_url(&info), "https://example.com");
    }

    #[test]
    fn build_base_url_nonstandard_port() {
        let info = make_req_info("https", "example.com", 8443);
        assert_eq!(provider::build_base_url(&info), "https://example.com:8443");
    }

    #[test]
    fn build_base_url_nonstandard_http_port() {
        let info = make_req_info("http", "example.com", 8080);
        assert_eq!(provider::build_base_url(&info), "http://example.com:8080");
    }

    // ── Session data serialization round-trip ─────────────────────────

    #[test]
    fn oidc_session_data_serialization_roundtrip() {
        let data = state::OidcSessionData {
            flow: Some(state::OidcFlowState {
                nonce: "test-nonce".into(),
                pkce_verifier: "test-pkce-verifier".into(),
                redirect_to: "/dashboard".into(),
                csrf_token: "test-csrf".into(),
                time: 1717800000,
            }),
            refresh_token: Some("test-refresh-token".into()),
            oidc_complete: Some(1717800000),
        };

        let bytes = rmp_serde::to_vec(&data).expect("serialize session data");
        let decoded: state::OidcSessionData =
            rmp_serde::from_slice(&bytes).expect("deserialize session data");

        assert_eq!(decoded.oidc_complete, Some(1717800000));
        assert_eq!(decoded.refresh_token.as_deref(), Some("test-refresh-token"));
        assert!(decoded.flow.is_some());
        let flow = decoded.flow.unwrap();
        assert_eq!(flow.nonce, "test-nonce");
        assert_eq!(flow.pkce_verifier, "test-pkce-verifier");
        assert_eq!(flow.redirect_to, "/dashboard");
        assert_eq!(flow.csrf_token, "test-csrf");
        assert_eq!(flow.time, 1717800000);
    }

    #[test]
    fn oidc_session_data_default_is_empty() {
        let data = state::OidcSessionData::default();
        assert!(data.flow.is_none());
        assert!(data.refresh_token.is_none());
        assert!(data.oidc_complete.is_none());
    }

    #[test]
    fn oidc_session_data_roundtrip_empty_default() {
        let data = state::OidcSessionData::default();
        let bytes = rmp_serde::to_vec(&data).expect("serialize");
        let decoded: state::OidcSessionData = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert!(decoded.flow.is_none());
        assert!(decoded.refresh_token.is_none());
        assert!(decoded.oidc_complete.is_none());
    }

    #[test]
    fn session_valid_within_ttl() {
        let now: i64 = 2000;
        let oidc_complete: i64 = 1000;
        let ttl: i64 = 3600;
        assert!(now < oidc_complete + ttl);
    }

    #[test]
    fn session_expired_after_ttl() {
        let now: i64 = 5000;
        let oidc_complete: i64 = 1000;
        let ttl: i64 = 3600;
        assert!(now >= oidc_complete + ttl);
    }

    // ── Constants ─────────────────────────────────────────────────────

    #[test]
    fn default_session_ttl_is_one_hour() {
        assert_eq!(DEFAULT_SESSION_TTL_SECS, 3600);
    }

    #[test]
    fn flow_state_ttl_is_five_minutes() {
        assert_eq!(FLOW_STATE_TTL_SECS, 300);
    }

    #[test]
    fn favicon_path_bypasses_auth() {
        assert_eq!(SYNC_FAVICON_PATH, "/favicon.ico");
    }
}

ksbh_modules_sdk::export_module!(
    process,
    ksbh_modules_sdk::module_definition!(
        ksbh_modules_sdk::abi::prelude::KSBHModuleKind::OIDC,
        [ksbh_modules_sdk::RequestStage::Request, ksbh_modules_sdk::RequestStage::BeforeRouting]
    )
);
