#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("validation error: {0}")]
    ValidationError(&'static str),
    #[error("missing mandatory value: {0}")]
    MissingMandatoryValue(String),
    #[error("configuration error: {0}")]
    ConfError(#[from] config::ConfigError),
    #[error("parsing error: {0}")]
    ParsingError(String),
}

impl From<Box<dyn ::std::error::Error + 'static>> for ConfigError {
    fn from(value: Box<dyn ::std::error::Error + 'static>) -> Self {
        Self::ParsingError(value.to_string())
    }
}
