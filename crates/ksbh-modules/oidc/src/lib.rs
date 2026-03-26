//! OpenID Connect authentication module.
//!
//! Full OIDC authorization code flow with PKCE support.
//! Checks session validity via `oidc_complete` timestamp.
//! Handles token refresh.
//! Stores flow state in namespaced session storage.
//! CSRF/state expiry after 5 minutes.

const DEFAULT_SESSION_TTL_SECS: u64 = 3600;
const FLOW_STATE_TTL_SECS: u64 = 300;
const FIVE_MINUTES: i64 = 300;
const CACHE_METADATA_TTL: ::std::time::Duration = ::std::time::Duration::from_hours(24);
static SYNC_FAVICON_PATH: &str = "/favicon.ico";
static DEFAULT_INTERNAL_PATH: &str = "/_ksbh_internal";
const DEFAULT_DISCOVERY_TIMEOUT_SECS: u64 = 30;
const MODULE_NAME: &str = "oidc";

static HTTP_CLIENT: ::std::sync::LazyLock<Result<reqwest::blocking::Client, &'static str>> =
    ::std::sync::LazyLock::new(|| {
        reqwest::blocking::ClientBuilder::new()
            .timeout(::std::time::Duration::from_secs(
                DEFAULT_DISCOVERY_TIMEOUT_SECS,
            ))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Failed to create blocking HTTP client for OIDC")
    });

type CachedMetadata = (
    openidconnect::core::CoreProviderMetadata,
    ::std::time::Instant,
);

static PROVIDER_METADATA_CACHE: ::std::sync::LazyLock<
    scc::HashMap<::std::string::String, CachedMetadata>,
> = ::std::sync::LazyLock::new(scc::HashMap::new);

#[derive(Debug)]
struct OIDCConfig<'a> {
    issuer_url: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct OidcSessionData {
    flow: Option<OidcFlowState>,
    refresh_token: Option<String>,
    oidc_complete: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct OidcFlowState {
    nonce: String,
    pkce_verifier: String,
    redirect_to: String,
    csrf_token: String,
    time: i64,
}

fn get_http_client() -> Result<&'static reqwest::blocking::Client, &'static str> {
    (*HTTP_CLIENT).as_ref().map_err(|e| *e)
}

const MAX_DISCOVERY_RETRIES: u32 = 2;
const DISCOVERY_RETRY_BASE_MS: u64 = 500;

fn get_or_cache_metadata(
    issuer_url: &str,
    http_client: &reqwest::blocking::Client,
) -> Result<openidconnect::core::CoreProviderMetadata, &'static str> {
    let now = ::std::time::Instant::now();

    let cache = &*PROVIDER_METADATA_CACHE;
    if let Some(entry) = cache.get_sync(issuer_url)
        && entry.1 + CACHE_METADATA_TTL > now
    {
        tracing::debug!("OIDC: using cached provider metadata for {}", issuer_url);
        return Ok(entry.0.clone());
    }

    let oidc_issuer_url =
        openidconnect::IssuerUrl::new(issuer_url.to_string()).map_err(|_| "Invalid issuer URL")?;

    let mut last_err = None;
    for attempt in 0..=MAX_DISCOVERY_RETRIES {
        if attempt > 0 {
            let backoff_ms = DISCOVERY_RETRY_BASE_MS * (1 << (attempt - 1));
            tracing::warn!(
                "OIDC: discovery retry {} for {} (backoff {}ms)",
                attempt,
                issuer_url,
                backoff_ms
            );
            ::std::thread::sleep(::std::time::Duration::from_millis(backoff_ms));
        }

        match openidconnect::core::CoreProviderMetadata::discover(&oidc_issuer_url, http_client) {
            Ok(provider_metadata) => {
                cache.upsert_sync(issuer_url.to_string(), (provider_metadata.clone(), now));
                tracing::debug!("OIDC: cached new provider metadata for {}", issuer_url);
                return Ok(provider_metadata);
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }

    if let Some(entry) = cache.get_sync(issuer_url) {
        return Ok(entry.0.clone());
    }

    tracing::error!(
        "OIDC: discovery failed for {} after {} retries: {:?}",
        issuer_url,
        MAX_DISCOVERY_RETRIES,
        last_err
    );
    Err("Failed to discover OIDC provider metadata")
}

fn build_base_url(req_info: &ksbh_modules_sdk::RequestInfo) -> String {
    let scheme = req_info.scheme.as_str();
    let host = req_info.host.as_str();
    let port = req_info.port;

    let is_standard =
        (scheme == "https" && port == 443) || (scheme == "http" && port == 80) || port == 0;

    if is_standard {
        format!("{}://{}", scheme, host)
    } else {
        format!("{}://{}:{}", scheme, host, port)
    }
}

fn load_session_data(session: &ksbh_modules_sdk::session::SessionHandle) -> OidcSessionData {
    match session.get(MODULE_NAME) {
        Some(bytes) => rmp_serde::from_slice(&bytes).unwrap_or(OidcSessionData {
            flow: None,
            refresh_token: None,
            oidc_complete: None,
        }),
        None => OidcSessionData {
            flow: None,
            refresh_token: None,
            oidc_complete: None,
        },
    }
}

fn save_session_data(
    session: &ksbh_modules_sdk::session::SessionHandle,
    data: &OidcSessionData,
    ttl_secs: i64,
) -> bool {
    match rmp_serde::to_vec(data) {
        Ok(bytes) => session.set_with_ttl(MODULE_NAME, &bytes, ttl_secs as u64),
        Err(_) => false,
    }
}

#[allow(clippy::type_complexity)]
fn build_oidc_client(
    config: &OIDCConfig,
    redirect_url: &str,
) -> Result<
    openidconnect::Client<
        openidconnect::EmptyAdditionalClaims,
        openidconnect::core::CoreAuthDisplay,
        openidconnect::core::CoreGenderClaim,
        openidconnect::core::CoreJweContentEncryptionAlgorithm,
        openidconnect::core::CoreJsonWebKey,
        openidconnect::core::CoreAuthPrompt,
        openidconnect::StandardErrorResponse<openidconnect::core::CoreErrorResponseType>,
        openidconnect::StandardTokenResponse<
            openidconnect::IdTokenFields<
                openidconnect::EmptyAdditionalClaims,
                openidconnect::EmptyExtraTokenFields,
                openidconnect::core::CoreGenderClaim,
                openidconnect::core::CoreJweContentEncryptionAlgorithm,
                openidconnect::core::CoreJwsSigningAlgorithm,
            >,
            openidconnect::core::CoreTokenType,
        >,
        openidconnect::StandardTokenIntrospectionResponse<
            openidconnect::EmptyExtraTokenFields,
            openidconnect::core::CoreTokenType,
        >,
        openidconnect::core::CoreRevocableToken,
        openidconnect::StandardErrorResponse<openidconnect::RevocationErrorResponseType>,
        openidconnect::EndpointSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointNotSet,
        openidconnect::EndpointMaybeSet,
        openidconnect::EndpointMaybeSet,
    >,
    &'static str,
> {
    let http_client = get_http_client().map_err(|_| "Failed to get HTTP client")?;
    let provider_metadata = get_or_cache_metadata(config.issuer_url, http_client)?;

    let client = openidconnect::core::CoreClient::from_provider_metadata(
        provider_metadata,
        openidconnect::ClientId::new(config.client_id.to_string()),
        Some(openidconnect::ClientSecret::new(
            config.client_secret.to_string(),
        )),
    )
    .set_redirect_uri(
        openidconnect::RedirectUrl::new(redirect_url.to_string())
            .map_err(|_| "Invalid redirect URL")?,
    );

    Ok(client)
}

fn get_authorization_code(
    config: &OIDCConfig,
    redirect_url: &str,
    original_uri: &str,
    session: &ksbh_modules_sdk::session::SessionHandle,
    session_data: &mut OidcSessionData,
) -> Result<String, &'static str> {
    let client = build_oidc_client(config, redirect_url)?;

    let (pkce_challenge, pkce_verifier) = openidconnect::PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            openidconnect::core::CoreAuthenticationFlow::AuthorizationCode,
            openidconnect::CsrfToken::new_random,
            openidconnect::Nonce::new_random,
        )
        .set_pkce_challenge(pkce_challenge)
        .url();

    session_data.flow = Some(OidcFlowState {
        nonce: nonce.secret().to_string(),
        pkce_verifier: pkce_verifier.into_secret(),
        redirect_to: original_uri.to_string(),
        csrf_token: csrf_token.secret().to_string(),
        time: ksbh_core::utils::current_unix_time(),
    });
    session_data.oidc_complete = None;

    if !save_session_data(session, session_data, FLOW_STATE_TTL_SECS as i64) {
        tracing::error!("OIDC: failed to save flow state to session store");
        return Err("Failed to save flow state");
    }

    Ok(auth_url.to_string())
}

/// Exchanges the authorization code for tokens.
/// Returns the refresh token if present.
fn exchange_token(
    config: &OIDCConfig,
    redirect_url: &str,
    code: &str,
    flow: &OidcFlowState,
) -> Result<Option<String>, &'static str> {
    let http_client = get_http_client().map_err(|_| "Failed to get HTTP client")?;
    let client = build_oidc_client(config, redirect_url)?;

    let code = openidconnect::AuthorizationCode::new(code.to_string());

    let token_response = client
        .exchange_code(code)
        .map_err(|_| "Token exchange failed")?
        .set_pkce_verifier(openidconnect::PkceCodeVerifier::new(
            flow.pkce_verifier.clone(),
        ))
        .request(http_client)
        .map_err(|_| "Token request failed")?;

    let id_token = token_response
        .extra_fields()
        .id_token()
        .ok_or("No ID token in response")?;

    id_token
        .claims(
            &client.id_token_verifier(),
            &openidconnect::Nonce::new(flow.nonce.clone()),
        )
        .map_err(|e| {
            tracing::error!("ID Token claims verification failed: {:?}", e);
            "ID token verification failed"
        })?;

    use openidconnect::OAuth2TokenResponse;
    let refresh_token = token_response
        .refresh_token()
        .map(|rt| rt.secret().to_string());

    Ok(refresh_token)
}

fn handle_auth_code_result(
    result: Result<String, &'static str>,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    match result {
        Ok(auth_url) => write_redirect(&auth_url),
        Err(_) => write_error(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Authorization failed",
        ),
    }
}

fn try_refresh_token(
    config: &OIDCConfig,
    redirect_url: &str,
    refresh_token: &str,
) -> Result<Option<String>, &'static str> {
    let http_client = get_http_client().map_err(|_| "Failed to get HTTP client")?;
    let client = build_oidc_client(config, redirect_url)?;

    let token_result = client
        .exchange_refresh_token(&openidconnect::RefreshToken::new(refresh_token.to_string()))
        .map_err(|_| "Token refresh failed")?
        .request(http_client)
        .map_err(|_| "Token refresh request failed")?;

    use openidconnect::OAuth2TokenResponse;
    let new_refresh_token = token_result
        .refresh_token()
        .map(|rt| rt.secret().to_string())
        .unwrap_or_else(|| refresh_token.to_string());

    Ok(Some(new_refresh_token))
}

fn write_redirect(
    location: &str,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let response = http::Response::builder()
        .status(http::StatusCode::FOUND)
        .header("Location", location)
        .header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate, max-age=0",
        )
        .body(bytes::Bytes::new())?;
    Ok(ksbh_modules_sdk::ModuleResult::Stop(response))
}

fn write_error(
    status: http::StatusCode,
    message: &str,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    Ok(ksbh_modules_sdk::ModuleResult::Stop(
        http::Response::builder()
            .status(status)
            .body(bytes::Bytes::from(message.to_string()))?,
    ))
}

fn is_unauthenticated_websocket_upgrade(is_websocket_handshake: bool, session_valid: bool) -> bool {
    !session_valid && is_websocket_handshake
}

pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let issuer_url = match ctx.config.get("issuer_url") {
        Some(v) => v.as_str(),
        None => {
            return write_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Missing issuer_url config",
            );
        }
    };
    let client_id = match ctx.config.get("client_id") {
        Some(v) => v.as_str(),
        None => {
            return write_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Missing client_id config",
            );
        }
    };
    let client_secret = match ctx.config.get("client_secret") {
        Some(v) => v.as_str(),
        None => {
            return write_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Missing client_secret config",
            );
        }
    };

    let session_ttl_secs = ctx
        .config
        .get("session_ttl_seconds")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_SESSION_TTL_SECS as i64);

    let enable_refresh = ctx
        .config
        .get("enable_refresh")
        .map(|v| v.as_str() == "true")
        .unwrap_or(false);

    let config = OIDCConfig {
        issuer_url,
        client_id,
        client_secret,
    };

    let base_url = build_base_url(&ctx.request);
    let path = ctx.request.path.as_str();
    let now = ksbh_core::utils::current_unix_time();

    if path == SYNC_FAVICON_PATH {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let mut session_data = load_session_data(&ctx.session);

    let session_valid = session_data
        .oidc_complete
        .map(|oidc_complete| now < oidc_complete + session_ttl_secs)
        .unwrap_or(false);

    if is_unauthenticated_websocket_upgrade(ctx.request.is_websocket_handshake, session_valid) {
        return write_error(http::StatusCode::UNAUTHORIZED, "OIDC required");
    }

    if session_valid {
        return Ok(ksbh_modules_sdk::ModuleResult::Pass);
    }

    let oidc_expired = session_data.oidc_complete.is_some();

    let modules_internal_path = ctx
        .config
        .get("modules_internal_path")
        .map(|s| s.as_str())
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
        match try_refresh_token(&config, &redirect_url, refresh_token) {
            Ok(new_refresh_token) => {
                session_data.refresh_token = new_refresh_token;
                session_data.flow = None;
                session_data.oidc_complete = Some(now);
                save_session_data(&ctx.session, &session_data, session_ttl_secs);

                let response = http::Response::builder()
                    .status(http::StatusCode::OK)
                    .body(bytes::Bytes::new())?;
                return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
            }
            Err(_) => {
                session_data.refresh_token = None;
            }
        }
    }

    if !path.starts_with(&module_path) || oidc_expired {
        let uri = ctx.request.uri.as_str();
        let result =
            get_authorization_code(&config, &redirect_url, uri, &ctx.session, &mut session_data);
        return handle_auth_code_result(result);
    }

    let code = ctx.request.query_params.get("code").map(|s| s.as_str());
    let code = match code {
        Some(c) => c,
        None => {
            return write_error(http::StatusCode::BAD_REQUEST, "Missing code parameter");
        }
    };
    let state = ctx.request.query_params.get("state").map(|s| s.as_str());
    let state = match state {
        Some(s) => s,
        None => {
            return write_error(http::StatusCode::BAD_REQUEST, "Missing state parameter");
        }
    };

    let flow = match &session_data.flow {
        Some(flow) => flow.clone(),
        None => {
            return write_error(
                http::StatusCode::BAD_REQUEST,
                "OIDC flow state not found in session",
            );
        }
    };

    if flow.csrf_token != state {
        return write_error(http::StatusCode::BAD_REQUEST, "Invalid state parameter");
    }

    if now > flow.time + FIVE_MINUTES {
        let original_uri = flow.redirect_to.clone();
        let result = get_authorization_code(
            &config,
            &redirect_url,
            &original_uri,
            &ctx.session,
            &mut session_data,
        );
        return handle_auth_code_result(result);
    }

    match exchange_token(&config, &redirect_url, code, &flow) {
        Ok(refresh_token) => {
            session_data.flow = None;
            session_data.refresh_token = refresh_token;
            save_session_data(&ctx.session, &session_data, session_ttl_secs);
        }
        Err(_) => {
            return write_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Token exchange failed",
            );
        }
    }

    let original_redirect = flow.redirect_to.clone();
    session_data.oidc_complete = Some(now);
    save_session_data(&ctx.session, &session_data, session_ttl_secs);

    write_redirect(&original_redirect)
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::Oidc);

#[cfg(test)]
mod tests {
    #[test]
    fn unauthenticated_upgrade_requires_websocket_handshake() {
        assert!(!super::is_unauthenticated_websocket_upgrade(false, false));
    }

    #[test]
    fn unauthenticated_valid_websocket_handshake_is_blocked() {
        assert!(super::is_unauthenticated_websocket_upgrade(true, false));
    }

    #[test]
    fn authenticated_websocket_handshake_is_not_blocked() {
        assert!(!super::is_unauthenticated_websocket_upgrade(true, true));
    }
}
