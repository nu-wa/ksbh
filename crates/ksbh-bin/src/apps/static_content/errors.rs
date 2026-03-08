#[derive(Debug)]
pub enum StaticHttpAppError {
    Internal(ksbh_types::KsbhStr),
}

impl ::std::error::Error for StaticHttpAppError {}

impl ::std::fmt::Display for StaticHttpAppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StaticHttpAppError {}",
            match self {
                Self::Internal(m) => m,
            }
        )
    }
}

impl From<Box<pingora::Error>> for StaticHttpAppError {
    fn from(value: Box<pingora::Error>) -> Self {
        Self::Internal(ksbh_types::KsbhStr::new(value.to_string()))
    }
}

impl From<ksbh_types::prelude::HttpRequestError> for StaticHttpAppError {
    fn from(value: ksbh_types::prelude::HttpRequestError) -> Self {
        Self::Internal(ksbh_types::KsbhStr::new(value.to_string()))
    }
}

impl From<askama::Error> for StaticHttpAppError {
    fn from(value: askama::Error) -> Self {
        Self::Internal(ksbh_types::KsbhStr::new(value.to_string()))
    }
}
