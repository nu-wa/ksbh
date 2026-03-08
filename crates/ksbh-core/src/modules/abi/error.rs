#[derive(Debug)]
pub enum AbiError {
    ModuleInstanceError(super::module_instance::ModuleInstanceError),
}

impl ::std::error::Error for AbiError {}

impl ::std::fmt::Display for AbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AbiError {}",
            match self {
                Self::ModuleInstanceError(m) => m,
            }
        )
    }
}

impl From<super::module_instance::ModuleInstanceError> for AbiError {
    fn from(value: super::module_instance::ModuleInstanceError) -> Self {
        Self::ModuleInstanceError(value)
    }
}
