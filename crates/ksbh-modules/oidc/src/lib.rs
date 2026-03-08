use ksbh_modules_sdk::{log_debug, log_error, ModuleResult, RequestContext};

const DEFAULT_SESSION_TTL_SECS: u64 = 3600;
const FLOW_STATE_TTL_SECS: u64 = 300;
const FIVE_MINUTES: ::std::time::Duration = ::std::time::Duration::from_mins(5);
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
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct OidcFlowState {
    nonce: String,
    pkce_verifier: String,
    redirect_to: String,
    csrf_token: String,
    time: chrono::NaiveDateTime,
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
        tracing::warn!(
            "OIDC: discovery failed for {}, serving stale cached metadata",
            issuer_url
        );
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
        }),
        None => OidcSessionData {
            flow: None,
            refresh_token: None,
        },
    }
}

fn save_session_data(
    session: &ksbh_modules_sdk::session::SessionHandle,
    data: &OidcSessionData,
    ttl_secs: u64,
) -> bool {
    match rmp_serde::to_vec(data) {
        Ok(bytes) => session.set_with_ttl(MODULE_NAME, &bytes, ttl_secs),
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
    cookie: &mut ksbh_core::cookies::ProxyCookie,
    session: &ksbh_modules_sdk::session::SessionHandle,
    session_data: &mut OidcSessionData,
) -> Result<(String, String), &'static str> {
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
        time: chrono::Local::now().naive_local(),
    });

    if !save_session_data(session, session_data, FLOW_STATE_TTL_SECS) {
        tracing::error!("OIDC: failed to save flow state to session store");
        return Err("Failed to save flow state");
    }

    cookie.oidc_complete = None;

    let cookie_header = cookie
        .to_cookie_header()
        .map_err(|_| "Failed to encode cookie")?;

    Ok((auth_url.to_string(), cookie_header))
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

fn write_redirect(location: &str, cookie_header: &str) -> ModuleResult {
    let response = http::Response::builder()
        .status(http::StatusCode::FOUND)
        .header("Location", location)
        .header("Set-Cookie", cookie_header)
        .header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate, max-age=0",
        )
        .body(bytes::Bytes::new())
        .unwrap();
    ModuleResult::Stop(response)
}

fn write_error(status: http::StatusCode, message: &str) -> ModuleResult {
    let response = http::Response::builder()
        .status(status)
        .body(bytes::Bytes::from(message.to_string()))
        .unwrap();
    ModuleResult::Stop(response)
}

pub fn process(ctx: RequestContext) -> ModuleResult {
    let logger = ctx.logger;

    log_debug!(logger, "===== request_filter START =====");
    let path = ctx.request.path.as_str();
    log_debug!(logger, "path = {}", path);

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
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SESSION_TTL_SECS);
    let session_ttl = ::std::time::Duration::from_secs(session_ttl_secs);

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
    let now = chrono::Local::now().naive_local();

    let host = ctx.request.host.as_str();

    let cookie_header = ctx.headers.get("Cookie").and_then(|h| h.to_str().ok());
    log_debug!(logger, "Cookie header received: {:?}", cookie_header);

    let mut cookie = match cookie_header {
        Some(header) => {
            log_debug!(logger, "Parsing cookie header: {}", header);
            match ksbh_core::cookies::ProxyCookie::from_cookie_header(header) {
                Ok(c) => {
                    log_debug!(
                        logger,
                        "Cookie parsed, oidc_complete: {:?}",
                        c.oidc_complete
                    );
                    c
                }
                Err(_e) => {
                    log_debug!(
                        logger,
                        "Failed to parse cookie: {:?}, creating new cookie",
                        _e
                    );
                    ksbh_core::cookies::ProxyCookie::new(host, None, uuid::Uuid::new_v4())
                }
            }
        }
        None => {
            log_debug!(logger, "No cookie header, creating new cookie");
            ksbh_core::cookies::ProxyCookie::new(host, None, uuid::Uuid::new_v4())
        }
    };

    if path == SYNC_FAVICON_PATH {
        return ModuleResult::Pass;
    }

    let session_valid = cookie
        .oidc_complete
        .map(|oidc_complete| now < oidc_complete + session_ttl)
        .unwrap_or(false);

    if ctx.headers.get("Upgrade").is_some() && !session_valid {
        return write_error(http::StatusCode::UNAUTHORIZED, "OIDC required");
    }

    if session_valid {
        return ModuleResult::Pass;
    }

    let mut session_data = load_session_data(&ctx.session);
    let oidc_expired = cookie.oidc_complete.is_some();

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

    if oidc_expired && enable_refresh
        && let Some(ref refresh_token) = session_data.refresh_token
    {
        log_debug!(logger, "attempting token refresh");
        match try_refresh_token(&config, &redirect_url, refresh_token) {
            Ok(new_refresh_token) => {
                session_data.refresh_token = new_refresh_token;
                session_data.flow = None;
                save_session_data(&ctx.session, &session_data, session_ttl_secs);

                cookie.oidc_complete = Some(now);
                let cookie_header = match cookie.to_cookie_header() {
                    Ok(h) => h,
                    Err(_) => {
                        return write_error(
                            http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Cookie encoding error",
                        );
                    }
                };

                let response = http::Response::builder()
                    .status(http::StatusCode::OK)
                    .header("Set-Cookie", cookie_header)
                    .body(bytes::Bytes::new())
                    .unwrap();
                return ModuleResult::Stop(response);
            }
            Err(_) => {
                log_debug!(logger, "token refresh failed, falling back to full auth");
                session_data.refresh_token = None;
            }
        }
    }

    if !path.starts_with(&module_path) || oidc_expired {
        log_debug!(logger, "initiating authorization flow");
        let uri = ctx.request.uri.as_str();
        match get_authorization_code(
            &config,
            &redirect_url,
            uri,
            &mut cookie,
            &ctx.session,
            &mut session_data,
        ) {
            Ok((auth_url, cookie_header)) => {
                return write_redirect(&auth_url, &cookie_header);
            }
            Err(_e) => {
                return write_error(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Authorization failed",
                );
            }
        };
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
        log_debug!(logger, "state expired, re-initiating flow");
        let original_uri = flow.redirect_to.clone();
        match get_authorization_code(
            &config,
            &redirect_url,
            &original_uri,
            &mut cookie,
            &ctx.session,
            &mut session_data,
        ) {
            Ok((auth_url, cookie_header)) => {
                return write_redirect(&auth_url, &cookie_header);
            }
            Err(_) => {
                return write_error(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Authorization failed",
                );
            }
        };
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
    cookie.oidc_complete = Some(now);

    let cookie_header = match cookie.to_cookie_header() {
        Ok(h) => h,
        Err(ref e) => {
            log_error!(logger, "Cookie header failed: {}", e);
            return write_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Cookie encoding error",
            );
        }
    };

    write_redirect(&original_redirect, &cookie_header)
}

ksbh_modules_sdk::register_module!(process, ksbh_modules_sdk::types::ModuleType::Oidc);
