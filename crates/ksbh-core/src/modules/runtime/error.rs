#[derive(Debug, thiserror::Error)]
pub enum AbiError {
    #[error(transparent)]
    ModuleInstanceError(#[from] super::module_instance::ModuleInstanceError),
}
