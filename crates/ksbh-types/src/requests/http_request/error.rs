#[derive(Debug)]
pub enum HttpRequestError {
    InvalidRequest,
    InvalidString(String),
}

impl ::std::error::Error for HttpRequestError {}

impl ::std::fmt::Display for HttpRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HttpRequestError: {}",
            match self {
                Self::InvalidRequest => "Invalid Request",
                Self::InvalidString(s) => &s,
            }
        )
    }
}

impl From<http::header::ToStrError> for HttpRequestError {
    fn from(value: http::header::ToStrError) -> Self {
        Self::InvalidString(value.to_string())
    }
}
