#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProxyCookie {
    pub challenge_complete: Option<i64>,
    pub session_id: uuid::Uuid,
    pub oidc_complete: Option<i64>,
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
    pub fn new(domain: &str, oidc_complete: Option<i64>, session_id: uuid::Uuid) -> Self {
        Self {
            challenge_complete: None,
            oidc_complete,
            session_id,
            domain: domain.to_string(),
        }
    }

    pub async fn from_session(
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
    ) -> Result<Self, ProxyCookieError> {
        use base64::Engine;

        let mut jar = cookie::CookieJar::new();

        for cookie in session.headers().headers.get_all(http::header::COOKIE) {
            let cookie_str = std::str::from_utf8(cookie.as_bytes())
                .map_err(|_| ProxyCookieError::CookieError("Invalid UTF-8".into()))?;

            for cookie in cookie::Cookie::split_parse(cookie_str).flatten() {
                jar.add_original(cookie.into_owned());
            }
        }

        let cookie = jar
            .private(&crate::COOKIE_ENC_KEY)
            .get(&crate::COOKIE_NAME)
            .ok_or(ProxyCookieError::NoCookie)?;

        let cookie_bytes = base64::prelude::BASE64_STANDARD_NO_PAD
            .decode(cookie.value().as_bytes())
            .map_err(|_| ProxyCookieError::CookieError("Base64 Decode error".into()))?;

        Ok(rmp_serde::from_slice(&cookie_bytes)?)
    }

    pub fn from_cookie_header(cookie_header: &str) -> Result<Self, ProxyCookieError> {
        use base64::Engine;

        let mut jar = cookie::CookieJar::new();

        for cookie in cookie::Cookie::split_parse(cookie_header).flatten() {
            jar.add_original(cookie.into_owned());
        }

        let cookie = jar
            .private(&crate::COOKIE_ENC_KEY)
            .get(&crate::COOKIE_NAME)
            .ok_or(ProxyCookieError::NoCookie)?;

        let cookie_bytes = base64::prelude::BASE64_STANDARD_NO_PAD
            .decode(cookie.value().as_bytes())
            .map_err(|_| ProxyCookieError::CookieError("Base64 Decode error".into()))?;

        Ok(rmp_serde::from_slice(&cookie_bytes)?)
    }

    pub fn to_cookie_header(&self) -> Result<String, ProxyCookieError> {
        use base64::Engine;

        let value_bytes = rmp_serde::to_vec(&self.to_owned())?;
        let value = base64::prelude::BASE64_STANDARD_NO_PAD.encode(value_bytes);

        let domain = get_cookie_domain(&self.domain);

        let mut jar = cookie::CookieJar::new();

        jar.private_mut(&crate::COOKIE_ENC_KEY).add(
            cookie::CookieBuilder::new(&*crate::COOKIE_NAME, value)
                .secure(true)
                .max_age(cookie::time::Duration::hours(24))
                .http_only(true)
                .same_site(cookie::SameSite::Lax)
                .path("/")
                .domain(domain),
        );

        let result = jar
            .get(&crate::COOKIE_NAME)
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

