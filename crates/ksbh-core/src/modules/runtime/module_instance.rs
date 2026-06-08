use ksbh_modules_abi::{
    functions::{ModuleFnFreeModuleResponse, ModuleFnGetDescriptor, ModuleFnHandleRequest},
    prelude::{
        KSBH_ABI_VERSION, KSBH_MAGIC, KSBHModuleKind, KSBHModuleStage, ModuleContext,
        ModuleResponse,
    },
};

/// Errors that can occur when loading or invoking a module instance.
#[derive(Debug, thiserror::Error)]
pub enum ModuleInstanceError {
    #[error("failed to load module library: {0}")]
    FailedToLoad(String),

    #[error("missing required module symbol: {0}")]
    MissingFunction(&'static str),

    #[error("invalid module descriptor: {0}")]
    InvalidDescriptor(String),

    #[error("incompatible module ABI version: module={module:?}, host={host:?}")]
    IncompatibleAbi {
        module: ksbh_modules_abi::version::KsbhAbiVersion,
        host: ksbh_modules_abi::version::KsbhAbiVersion,
    },
}

/// Loaded module instance representing a dynamically-loaded library.
/// Contains FFI function pointers and module metadata.
pub struct ModuleInstance {
    /// Holds the dynamic library handle. Never read directly — its RAII Drop
    /// keeps the library loaded. If removed, all FFI function pointers dangle.
    _library: libloading::Library,
    pub handle_request_fn: ModuleFnHandleRequest,
    pub free_module_response_fn: ModuleFnFreeModuleResponse,
    pub registered_stages: Vec<KSBHModuleStage>,
    pub(super) file_name: ::std::sync::Arc<str>,
    pub(super) mod_type: crate::modules::ModuleConfigurationType,
}

impl ModuleInstance {
    /// Loads a module from the given dynamic library path.
    /// Validates required FFI functions exist and retrieves module type.
    pub fn load<P: AsRef<::std::path::Path>>(path: P) -> Result<Self, ModuleInstanceError> {
        let path_ref = path.as_ref();

        let lib = unsafe {
            libloading::Library::new(path_ref).map_err(|e| {
                ModuleInstanceError::FailedToLoad(format!(
                    "Library::new failed for {:?}: {}",
                    path_ref, e
                ))
            })?
        };

        let module_descriptor_fn: ModuleFnGetDescriptor =
            unsafe { *lib.get(b"module_descriptor\0")? };

        let module_descriptor = unsafe { module_descriptor_fn() };

        if module_descriptor.magic != KSBH_MAGIC {
            return Err(ModuleInstanceError::InvalidDescriptor(
                "Wrong magic value".into(),
            ));
        }

        if !module_descriptor
            .abi_version
            .is_compatible_with_host(KSBH_ABI_VERSION)
        {
            return Err(ModuleInstanceError::IncompatibleAbi {
                module: module_descriptor.abi_version,
                host: KSBH_ABI_VERSION,
            }
            .into());
        }

        let mod_type = match module_descriptor.info.kind {
            KSBHModuleKind::OIDC => crate::modules::ModuleConfigurationType::OIDC,
            KSBHModuleKind::POW => crate::modules::ModuleConfigurationType::POW,
            KSBHModuleKind::RateLimit => crate::modules::ModuleConfigurationType::RateLimit,
            KSBHModuleKind::Robots => crate::modules::ModuleConfigurationType::RobotsDotTXT,
            KSBHModuleKind::HttpToHttps => crate::modules::ModuleConfigurationType::HttpToHttps,
            KSBHModuleKind::Custom => {
                let mod_name_bytes = unsafe {
                    std::slice::from_raw_parts(
                        module_descriptor.info.name.inner.ptr,
                        module_descriptor.info.name.inner.len,
                    )
                };

                let mod_name = std::str::from_utf8(mod_name_bytes)
                    .map_err(|e| ModuleInstanceError::InvalidDescriptor(e.to_string()))?;

                if mod_name.is_empty() {
                    return Err(ModuleInstanceError::InvalidDescriptor(
                        "custom module kind requires non-empty name".into(),
                    ));
                }
                crate::modules::ModuleConfigurationType::Custom(mod_name.to_string())
            }
        };

        let mut registered_stages: Vec<KSBHModuleStage> = vec![];

        unsafe {
            let mod_stages = ::std::slice::from_raw_parts(
                module_descriptor.info.registered_stages.ptr,
                module_descriptor.info.registered_stages.len,
            );

            for &mod_stage in mod_stages {
                registered_stages.push(mod_stage);
            }
        }

        Ok(Self {
            _library: lib,
            registered_stages,
            file_name: path_ref.to_string_lossy().into(),
            free_module_response_fn: module_descriptor.free_module_response_fn,
            handle_request_fn: module_descriptor.handle_request_fn,
            mod_type,
        })
    }

    /// Calls the module's `request_filter` entry point with the given context.
    /// Returns a raw pointer to the module response; caller must use `free_response`.
    pub fn call_request_filter<'ctx>(
        &self,
        stage: KSBHModuleStage,
        ctx: &mut ModuleContext,
    ) -> *const ModuleResponse {
        if !stage_is_registered(&self.registered_stages, stage) {
            return std::ptr::null();
        }

        let ctx_ptr = ctx as *mut ModuleContext;

        unsafe { (self.handle_request_fn)(stage, ctx_ptr) }
    }

    /// # Safety
    ///
    /// The `resp` pointer must have been obtained from a call to `call_request_filter`
    /// on this same `ModuleInstance`. Calling this with a pointer from a different source
    /// or after the module instance has been dropped results in undefined behavior.
    pub unsafe fn free_response(&self, ctx: *mut ModuleContext, resp: *mut ModuleResponse) {
        unsafe { (self.free_module_response_fn)(ctx, resp) }
    }
}

impl From<libloading::Error> for ModuleInstanceError {
    fn from(value: libloading::Error) -> Self {
        Self::FailedToLoad(value.to_string())
    }
}

fn stage_is_registered(registered_stages: &[KSBHModuleStage], stage: KSBHModuleStage) -> bool {
    registered_stages.contains(&stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_is_registered_matches_declared_stages() {
        let registered_stages = vec![KSBHModuleStage::Request];
        assert!(stage_is_registered(
            &registered_stages,
            KSBHModuleStage::Request
        ));
        assert!(!stage_is_registered(
            &registered_stages,
            KSBHModuleStage::BeforeRouting
        ));
        assert!(!stage_is_registered(
            &registered_stages,
            KSBHModuleStage::Logging
        ));
    }
}
