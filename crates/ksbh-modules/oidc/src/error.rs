#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ModuleOIDCError {
    OIDCError(String),
    InvalidRequest(String),
    BadRequest(String),
    InternalServerError(String),
    Unauthorized,
}

impl ::std::error::Error for ModuleOIDCError {}

impl ::std::fmt::Display for ModuleOIDCError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ModuleOIDC<Error> '{}'",
            match self {
                Self::OIDCError(m) => m,
                Self::InvalidRequest(m) => m,
                Self::BadRequest(m) => m,
                Self::InternalServerError(m) => m,
                Self::Unauthorized => "Unauthorized",
            }
        )
    }
}

#[async_trait::async_trait]
impl ksbh_core::modules::ModuleError for ModuleOIDCError {
    async fn early_to_pingora(&self) -> (http::StatusCode, bytes::Bytes) {
        match self {
            ModuleOIDCError::InternalServerError(m) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                bytes::Bytes::copy_from_slice(format!("InternalServerError: {m}").as_bytes()),
            ),
            ModuleOIDCError::BadRequest(m) => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::copy_from_slice(format!("InvalidRequest: {m}").as_bytes()),
            ),
            ModuleOIDCError::InvalidRequest(m) => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::copy_from_slice(format!("InvalidRequest: {m}").as_bytes()),
            ),
            ModuleOIDCError::OIDCError(m) => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::copy_from_slice(format!("InvalidRequest: {m}").as_bytes()),
            ),
            ModuleOIDCError::Unauthorized => (
                http::StatusCode::UNAUTHORIZED,
                bytes::Bytes::copy_from_slice("Unauthorized".as_bytes()),
            ),
        }
    }

    async fn to_pingora(
        &self,
        pingora_session: &mut pingora::proxy::Session,
    ) -> pingora::Result<bool> {
        match self {
            ModuleOIDCError::InternalServerError(m) => {
                pingora_session
                    .respond_error_with_body(500, bytes::Bytes::copy_from_slice(m.as_bytes()))
                    .await?
            }
            ModuleOIDCError::BadRequest(m) => {
                let r = format!("InvalidRequest: {m}");
                pingora_session
                    .respond_error_with_body(400, bytes::Bytes::copy_from_slice(r.as_bytes()))
                    .await?
            }
            ModuleOIDCError::InvalidRequest(m) => {
                tracing::debug!("invalid request: {m}");
                let r = format!("InvalidRequest: {m}");
                pingora_session
                    .respond_error_with_body(400, bytes::Bytes::copy_from_slice(r.as_bytes()))
                    .await?
            }
            _ => {
                pingora_session
                    .respond_error_with_body(500, bytes::Bytes::from("InvalidRequest".as_bytes()))
                    .await?
            }
        };
        Ok(true)
    }
}

impl From<openidconnect::ClaimsVerificationError> for ModuleOIDCError {
    fn from(value: openidconnect::ClaimsVerificationError) -> Self {
        tracing::debug!("og err: {:?}", value);
        Self::OIDCError(value.to_string())
    }
}

impl From<::std::string::FromUtf8Error> for ModuleOIDCError {
    fn from(value: ::std::string::FromUtf8Error) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}

impl From<Box<pingora::Error>> for ModuleOIDCError {
    fn from(value: Box<pingora::Error>) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for ModuleOIDCError {
    fn from(value: jsonwebtoken::errors::Error) -> Self {
        tracing::debug!("og err: {:?}", value);
        Self::OIDCError(value.to_string())
    }
}

impl From<Box<dyn ::std::error::Error>> for ModuleOIDCError {
    fn from(value: Box<dyn ::std::error::Error>) -> Self {
        tracing::debug!("og err: {:?}", value);
        Self::InvalidRequest(value.to_string())
    }
}

impl From<ksbh_core::cookies::ProxyCookieError> for ModuleOIDCError {
    fn from(value: ksbh_core::cookies::ProxyCookieError) -> Self {
        Self::InternalServerError(value.to_string())
    }
}

impl From<ksbh_types::prelude::ProxyProviderError> for ModuleOIDCError {
    fn from(value: ksbh_types::prelude::ProxyProviderError) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}
