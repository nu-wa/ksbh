pub enum ModuleError {
    Response {
        status: http::StatusCode,
        message: String,
    },
    Critical(Box<dyn ::std::error::Error + Send + Sync>),
}

impl ModuleError {
    pub fn response(status: http::StatusCode, msg: impl Into<String>) -> Self {
        Self::Response {
            status,
            message: msg.into(),
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::response(http::StatusCode::BAD_REQUEST, msg)
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::response(http::StatusCode::UNAUTHORIZED, msg)
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::response(http::StatusCode::FORBIDDEN, msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::response(http::StatusCode::NOT_FOUND, msg)
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::response(http::StatusCode::INTERNAL_SERVER_ERROR, msg)
    }

    pub fn too_many_requests(msg: impl Into<String>) -> Self {
        Self::response(http::StatusCode::TOO_MANY_REQUESTS, msg)
    }

    pub fn critical<E: ::std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::Critical(Box::new(e))
    }
}

impl ::std::fmt::Debug for ModuleError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Response { status, message } => {
                write!(f, "ModuleError::Response({}, {})", status.as_u16(), message)
            }
            Self::Critical(e) => write!(f, "ModuleError::Critical({})", e),
        }
    }
}

impl ::std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::Response { message, .. } => write!(f, "{}", message),
            Self::Critical(e) => write!(f, "Critical: {}", e),
        }
    }
}

impl ::std::error::Error for ModuleError {}

// Implement From for specific error types
impl From<::std::io::Error> for ModuleError {
    fn from(e: ::std::io::Error) -> Self {
        Self::Critical(Box::new(e))
    }
}

impl From<http::Error> for ModuleError {
    fn from(e: http::Error) -> Self {
        Self::Response {
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        }
    }
}

impl From<::std::string::String> for ModuleError {
    fn from(s: ::std::string::String) -> Self {
        Self::Response {
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            message: s,
        }
    }
}

impl From<&str> for ModuleError {
    fn from(s: &str) -> Self {
        Self::Response {
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            message: s.to_string(),
        }
    }
}
