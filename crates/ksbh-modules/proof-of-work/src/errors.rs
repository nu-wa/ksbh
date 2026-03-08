#[derive(Debug)]
pub enum ModulePOWError {
    InvalidRequest(String),
    BadRequest(String),
    InternalServerError(String),
    TemplateError(String),
    ConfigError(String),
    Unauthorized,
}

impl ::std::error::Error for ModulePOWError {}

impl ::std::fmt::Display for ModulePOWError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ModulePOW<Error> '{}'",
            match self {
                Self::InvalidRequest(m) => m.to_string(),
                Self::BadRequest(m) => m.to_string(),
                Self::InternalServerError(m) => m.to_string(),
                Self::TemplateError(m) => m.to_string(),
                Self::ConfigError(m) => m.to_string(),
                Self::Unauthorized => "Unauthorized".to_string(),
            }
        )
    }
}

#[async_trait::async_trait]
impl ksbh_core::modules::ModuleError for ModulePOWError {
    async fn early_to_pingora(&self) -> (http::StatusCode, bytes::Bytes) {
        match self {
            Self::Unauthorized => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::copy_from_slice(b"Unauthorized"),
            ),
            Self::BadRequest(m) => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::copy_from_slice(m.as_bytes()),
            ),
            Self::InternalServerError(m) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                bytes::Bytes::copy_from_slice(m.as_bytes()),
            ),
            Self::TemplateError(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                bytes::Bytes::copy_from_slice(e.to_string().as_bytes()),
            ),
            Self::ConfigError(m) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                bytes::Bytes::copy_from_slice(m.as_bytes()),
            ),
            Self::InvalidRequest(m) => (
                http::StatusCode::BAD_REQUEST,
                bytes::Bytes::copy_from_slice(m.as_bytes()),
            ),
        }
    }

    async fn to_pingora(
        &self,
        pingora_session: &mut pingora::proxy::Session,
    ) -> pingora::Result<bool> {
        match self {
            ModulePOWError::InternalServerError(m) => {
                pingora_session
                    .respond_error_with_body(500, bytes::Bytes::copy_from_slice(m.as_bytes()))
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

impl From<::std::string::FromUtf8Error> for ModulePOWError {
    fn from(value: ::std::string::FromUtf8Error) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}

impl From<Box<pingora::Error>> for ModulePOWError {
    fn from(value: Box<pingora::Error>) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}

impl From<Box<dyn ::std::error::Error>> for ModulePOWError {
    fn from(value: Box<dyn ::std::error::Error>) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}

impl From<::std::time::SystemTimeError> for ModulePOWError {
    fn from(value: ::std::time::SystemTimeError) -> Self {
        Self::InternalServerError(value.to_string())
    }
}

impl From<std::array::TryFromSliceError> for ModulePOWError {
    fn from(value: std::array::TryFromSliceError) -> Self {
        Self::InternalServerError(value.to_string())
    }
}

impl From<ksbh_core::cookies::ProxyCookieError> for ModulePOWError {
    fn from(value: ksbh_core::cookies::ProxyCookieError) -> Self {
        Self::InternalServerError(value.to_string())
    }
}

impl From<ksbh_types::prelude::ProxyProviderError> for ModulePOWError {
    fn from(value: ksbh_types::prelude::ProxyProviderError) -> Self {
        Self::InvalidRequest(value.to_string())
    }
}

impl From<askama::Error> for ModulePOWError {
    fn from(value: askama::Error) -> Self {
        Self::TemplateError(value.to_string())
    }
}
