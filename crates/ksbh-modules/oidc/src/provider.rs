use crate::state::OidcFlowState;

pub const DEFAULT_DISCOVERY_TIMEOUT_SECS: u64 = 30;
pub const CACHE_METADATA_TTL: ::std::time::Duration = ::std::time::Duration::from_hours(24);
const MAX_DISCOVERY_RETRIES: u32 = 2;
const DISCOVERY_RETRY_BASE_MS: u64 = 500;

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
pub struct OidcConfig<'a> {
    pub issuer_url: &'a str,
    pub client_id: &'a str,
    pub client_secret: &'a str,
}

pub fn build_base_url(req_info: &ksbh_modules_sdk::RequestInfo) -> String {
    let scheme = req_info.scheme;
    let host = req_info.host;
    let port = req_info.port;

    let is_standard =
        (scheme == "https" && port == 443) || (scheme == "http" && port == 80) || port == 0;

    if is_standard {
        format!("{scheme}://{host}")
    } else {
        format!("{scheme}://{host}:{port}")
    }
}

fn get_http_client() -> Result<&'static reqwest::blocking::Client, &'static str> {
    (*HTTP_CLIENT).as_ref().map_err(|e| *e)
}

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
            Err(e) => last_err = Some(e),
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

#[allow(clippy::type_complexity)]
pub fn build_oidc_client(
    config: &OidcConfig<'_>,
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

pub fn get_authorization_code(
    config: &OidcConfig<'_>,
    redirect_url: &str,
    original_uri: &str,
    session_data: &mut crate::state::OidcSessionData,
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

    Ok(auth_url.to_string())
}

pub fn exchange_token(
    config: &OidcConfig<'_>,
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

pub fn try_refresh_token(
    config: &OidcConfig<'_>,
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
