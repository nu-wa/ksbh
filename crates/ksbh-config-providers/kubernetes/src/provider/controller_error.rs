#[derive(Debug)]
pub enum ControllerError {
    KubeError(kube::Error),
    InvalidIngress(::std::string::String),
}

impl ::std::error::Error for ControllerError {}

impl ::std::fmt::Display for ControllerError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "ControllerError: '{}'",
            match self {
                ControllerError::KubeError(e) => e.to_string(),
                ControllerError::InvalidIngress(e) => e.to_string(),
            }
        )
    }
}

impl From<kube::Error> for ControllerError {
    fn from(value: kube::Error) -> Self {
        ControllerError::KubeError(value)
    }
}
