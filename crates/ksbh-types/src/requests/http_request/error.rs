#[derive(Debug)]
pub enum HttpRequestError {
    InvalidRequest,
    InvaidString(String),
}

impl ::std::error::Error for HttpRequestError {}

impl ::std::fmt::Display for HttpRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HttpRequestError: {}",
            match self {
                Self::InvalidRequest => "Invalid Request",
                Self::InvaidString(s) => &s,
            }
        )
    }
}

impl From<http::header::ToStrError> for HttpRequestError {
    fn from(value: http::header::ToStrError) -> Self {
        Self::InvaidString(value.to_string())
    }
}
