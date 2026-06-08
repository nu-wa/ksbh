#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    #[error("{message}")]
    Response {
        status: http::StatusCode,
        message: String,
    },

    #[error("host interaction failed: {message}")]
    Host { message: String },

    #[error("invalid ABI data: {message}")]
    Abi { message: String },

    #[error("critical module error: {0}")]
    Critical(#[source] anyhow::Error),
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

    pub fn host(message: impl Into<String>) -> Self {
        Self::Host {
            message: message.into(),
        }
    }

    pub fn abi(message: impl Into<String>) -> Self {
        Self::Abi {
            message: message.into(),
        }
    }

    pub fn missing_config(key: &str) -> Self {
        Self::abi(format!("missing required config `{key}`"))
    }

    pub fn critical<E>(error: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        Self::Critical(error.into())
    }
}

impl From<anyhow::Error> for ModuleError {
    fn from(error: anyhow::Error) -> Self {
        Self::Critical(error)
    }
}

impl From<std::io::Error> for ModuleError {
    fn from(error: std::io::Error) -> Self {
        Self::Critical(error.into())
    }
}

impl From<http::Error> for ModuleError {
    fn from(error: http::Error) -> Self {
        Self::critical(error)
    }
}

impl From<std::str::Utf8Error> for ModuleError {
    fn from(error: std::str::Utf8Error) -> Self {
        Self::Abi {
            message: error.to_string(),
        }
    }
}

impl From<std::num::ParseIntError> for ModuleError {
    fn from(error: std::num::ParseIntError) -> Self {
        Self::Abi {
            message: error.to_string(),
        }
    }
}

impl From<std::array::TryFromSliceError> for ModuleError {
    fn from(error: std::array::TryFromSliceError) -> Self {
        Self::Abi {
            message: error.to_string(),
        }
    }
}


impl From<String> for ModuleError {
    fn from(message: String) -> Self {
        Self::critical(anyhow::anyhow!(message))
    }
}

impl From<&str> for ModuleError {
    fn from(message: &str) -> Self {
        Self::critical(anyhow::anyhow!(message.to_string()))
    }
}
