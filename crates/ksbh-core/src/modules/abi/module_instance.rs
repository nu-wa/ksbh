#[derive(Debug)]
pub enum ModuleInstanceError {
    FailedToLoad(String),
    MissingFunction(&'static str),
}

#[derive(Debug)]
pub struct ModuleInstance {
    _lib: libloading::Library,
    request_filter_entry: super::ModuleEntryFn,
    free_response_fn_entry: super::ModuleResponseFreeFn,
    pub(super) file_name: ::std::sync::Arc<str>,
    pub(super) mod_type: crate::modules::ModuleConfigurationType,
}

impl ModuleInstance {
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

        let entry_fn: super::ModuleEntryFn = unsafe {
            *lib.get(b"request_filter\0").map_err(|e| {
                tracing::error!("Failed to find request_filter in {:?}: {}", path_ref, e);
                ModuleInstanceError::MissingFunction("request_filter")
            })?
        };

        let get_type_fn: super::ModuleGetTypeFn = unsafe {
            *lib.get(b"get_module_type\0").map_err(|e| {
                tracing::error!("Failed to find get_module_type in {:?}: {}", path_ref, e);
                ModuleInstanceError::MissingFunction("get_module_type")
            })?
        };

        let free_response_fn: super::ModuleResponseFreeFn = unsafe {
            *lib.get(b"free_response\0").map_err(|e| {
                tracing::error!("Failed to find free_response in {:?}: {}", path_ref, e);
                ModuleInstanceError::MissingFunction("free_response")
            })?
        };

        let mod_type = unsafe { crate::modules::ModuleConfigurationType::try_from(get_type_fn())? };

        Ok(Self {
            _lib: lib,
            request_filter_entry: entry_fn,
            free_response_fn_entry: free_response_fn,
            file_name: path_ref.to_string_lossy().into(),
            mod_type,
        })
    }

    pub fn call_request_filter(
        &self,
        ctx: &mut super::ModuleContext<'_>,
    ) -> *const super::ModuleResponse {
        let ctx_ptr = ctx as *mut super::ModuleContext<'_>;

        unsafe { (self.request_filter_entry)(ctx_ptr) }
    }

    /// # Safety
    ///
    /// The `resp` pointer must have been obtained from a call to `call_request_filter`
    /// on this same `ModuleInstance`. Calling this with a pointer from a different source
    /// or after the module instance has been dropped results in undefined behavior.
    pub unsafe fn free_response(&self, resp: *const super::ModuleResponse) {
        unsafe { (self.free_response_fn_entry)(resp) }
    }
}

impl ::std::error::Error for ModuleInstanceError {}

impl ::std::fmt::Display for ModuleInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ModuleInstanceError {}",
            match self {
                Self::MissingFunction(function_name) =>
                    format!("Missing function: {}", function_name),
                Self::FailedToLoad(m) => m.to_string(),
            }
        )
    }
}

impl From<libloading::Error> for ModuleInstanceError {
    fn from(value: libloading::Error) -> Self {
        match value {
            libloading::Error::GetProcAddress { source: _ } => Self::MissingFunction("unknown"),
            _ => Self::FailedToLoad(value.to_string()),
        }
    }
}

impl From<std::string::FromUtf8Error> for ModuleInstanceError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::FailedToLoad(value.to_string())
    }
}
