#[derive(Debug)]
pub struct CookieSettings {
    pub key: cookie::Key,
    pub name: ::std::string::String,
    pub secure: bool,
}

impl CookieSettings {
    pub fn from_config(config: &crate::Config) -> Result<Self, crate::config::ConfigError> {
        let cookie_key = config.cookie_key.as_ref().ok_or_else(|| {
            crate::config::ConfigError::MissingMandatoryValue(
                "cookie_key must be provided via config or KSBH__COOKIE_KEY".to_string(),
            )
        })?;

        let key = crate::cookie::Key::try_from(cookie_key.as_bytes()).map_err(|_| {
            crate::config::ConfigError::ValidationError("cookie_key must be at least 64 bytes")
        })?;

        Ok(Self {
            key,
            name: config.constants.cookie_name.clone(),
            secure: config.constants.cookie_secure,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProxyCookie {
    pub session_id: uuid::Uuid,
    domain: String,
}

pub fn get_cookie_domain(host: &str) -> String {
    if let Some(domain) = psl::domain(host.as_bytes()) {
        let domain_str = ::std::str::from_utf8(domain.as_bytes()).unwrap_or(host);
        return format!(".{}", domain_str);
    }
    format!(".{}", host)
}

#[derive(Debug)]
pub enum ProxyCookieError {
    CookieError(String),
    EncodeError(rmp_serde::encode::Error),
    DecodeError(rmp_serde::decode::Error),
    NoCookie,
}

impl ::std::error::Error for ProxyCookieError {}

impl ::std::fmt::Display for ProxyCookieError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProxyCookieError<{}>: '{}'.",
            match self {
                Self::NoCookie => "NoCookie",
                Self::CookieError(_) => "CookieError",
                Self::DecodeError(_) => "DecodeError",
                Self::EncodeError(_) => "EncodeError",
            },
            match self {
                Self::NoCookie => "No cookie header".to_string(),
                Self::CookieError(e) => e.to_string(),
                Self::DecodeError(e) => e.to_string(),
                Self::EncodeError(e) => e.to_string(),
            }
        )
    }
}

impl ProxyCookie {
    pub fn new(domain: &str, session_id: uuid::Uuid) -> Self {
        Self {
            session_id,
            domain: domain.to_string(),
        }
    }

    pub async fn from_session(
        cookie_settings: &CookieSettings,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
    ) -> Result<Self, ProxyCookieError> {
        use base64::Engine;

        let mut jar = cookie::CookieJar::new();

        for cookie in session.header_map().get_all(http::header::COOKIE) {
            let cookie_str = std::str::from_utf8(cookie.as_bytes())
                .map_err(|_| ProxyCookieError::CookieError("Invalid UTF-8".into()))?;

            for cookie in cookie::Cookie::split_parse(cookie_str).flatten() {
                jar.add_original(cookie.into_owned());
            }
        }

        let cookie = jar
            .private(&cookie_settings.key)
            .get(&cookie_settings.name)
            .ok_or(ProxyCookieError::NoCookie)?;

        let cookie_bytes = base64::prelude::BASE64_STANDARD_NO_PAD
            .decode(cookie.value().as_bytes())
            .map_err(|_| ProxyCookieError::CookieError("Base64 Decode error".into()))?;

        Ok(rmp_serde::from_slice(&cookie_bytes)?)
    }

    pub fn from_cookie_header(
        cookie_settings: &CookieSettings,
        cookie_header: &str,
    ) -> Result<Self, ProxyCookieError> {
        use base64::Engine;

        let mut jar = cookie::CookieJar::new();

        for cookie in cookie::Cookie::split_parse(cookie_header).flatten() {
            jar.add_original(cookie.into_owned());
        }

        let cookie = jar
            .private(&cookie_settings.key)
            .get(&cookie_settings.name)
            .ok_or(ProxyCookieError::NoCookie)?;

        let cookie_bytes = base64::prelude::BASE64_STANDARD_NO_PAD
            .decode(cookie.value().as_bytes())
            .map_err(|_| ProxyCookieError::CookieError("Base64 Decode error".into()))?;

        Ok(rmp_serde::from_slice(&cookie_bytes)?)
    }

    pub fn to_cookie_header(
        &self,
        cookie_settings: &CookieSettings,
    ) -> Result<String, ProxyCookieError> {
        use base64::Engine;

        let value_bytes = rmp_serde::to_vec(&self.to_owned())?;
        let value = base64::prelude::BASE64_STANDARD_NO_PAD.encode(value_bytes);

        let domain = get_cookie_domain(&self.domain);

        let mut jar = cookie::CookieJar::new();

        jar.private_mut(&cookie_settings.key).add(
            cookie::CookieBuilder::new(cookie_settings.name.clone(), value)
                .secure(cookie_settings.secure)
                .max_age(cookie::time::Duration::hours(24))
                .http_only(true)
                .same_site(cookie::SameSite::Lax)
                .path("/")
                .domain(domain),
        );

        let result = jar
            .get(&cookie_settings.name)
            .map(|c| c.to_string())
            .ok_or(ProxyCookieError::NoCookie)?;

        Ok(result)
    }
}

impl From<rmp_serde::decode::Error> for ProxyCookieError {
    fn from(value: rmp_serde::decode::Error) -> Self {
        Self::DecodeError(value)
    }
}

impl From<rmp_serde::encode::Error> for ProxyCookieError {
    fn from(value: rmp_serde::encode::Error) -> Self {
        Self::EncodeError(value)
    }
}
