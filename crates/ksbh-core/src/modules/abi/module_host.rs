#[derive(Debug, PartialEq)]
pub enum ModuleHostError {
    ModuleNotFound,
    ModuleError(String),
    InternalError(String),
}

impl ::std::error::Error for ModuleHostError {}

impl ::std::fmt::Display for ModuleHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ModuleHostError {}",
            match self {
                Self::ModuleNotFound => "Module not found".to_string(),
                Self::ModuleError(m) => m.to_string(),
                Self::InternalError(m) => m.to_string(),
            }
        )
    }
}

pub struct ModuleHost {
    modules: scc::HashMap<
        crate::modules::ModuleConfigurationType,
        super::module_instance::ModuleInstance,
    >,
    store: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
}

impl ModuleHost {
    pub fn new(
        store: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    ) -> Self {
        super::host_functions::set_module_sessions(store.clone());
        super::host_functions::set_module_metrics(store.clone());
        Self {
            modules: scc::HashMap::new(),
            store,
        }
    }

    pub fn session_store(
        &self,
    ) -> &::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    > {
        &self.store
    }
}

impl ModuleHost {
    pub fn load_module<P: AsRef<::std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), super::error::AbiError> {
        let module_instance = super::module_instance::ModuleInstance::load(path)?;

        if let Err((mod_type, existing_module_instance)) = self
            .modules
            .insert_sync(module_instance.mod_type.clone(), module_instance)
        {
            tracing::error!(
                "Module type already loaded {:?} (path: {})",
                mod_type,
                existing_module_instance.file_name
            );
        }

        Ok(())
    }

    pub async fn call_module(
        &self,
        req_ctx: &super::module_request_context::ModuleRequestContext<'_>,
        params: &super::module_request_context::ModuleCallParams,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
    ) -> Result<(), ModuleHostError> {
        let module = match self.modules.get_sync(&params.module_type) {
            Some(m) => m,
            None => return Err(ModuleHostError::ModuleNotFound),
        };

        let mut ctx = super::ModuleContext {
            config: &params.config_kv_slice,
            headers: &req_ctx.headers,
            req_info: &req_ctx.request_info,
            body: req_ctx.body.as_deref().unwrap_or(&[]),
            log_fn: super::host_functions::host_fn_log,
            session_id: req_ctx.session_id_bytes,
            session_get_fn: super::host_functions::host_session_get,
            session_set_fn: super::host_functions::host_session_set,
            session_set_with_ttl_fn: super::host_functions::host_session_set_with_ttl,
            session_free_fn: super::host_functions::host_session_free,
            mod_name: params.module_name.as_ref(),
            cookie_header: super::ModuleBuffer::from_ref(req_ctx.cookie_header.as_str()),
            metrics_key: super::ModuleBuffer::from_ref_bytes(req_ctx.metrics_key),
            metrics_good_boy_fn: super::host_functions::host_metrics_good_boy,
            metrics_get_score_fn: super::host_functions::host_metrics_get_score,
            internal_path: super::ModuleBuffer::from_ref(req_ctx.internal_path.as_str()),
        };

        let module_response = module.call_request_filter(&mut ctx);

        if module_response.is_null() {
            return Ok(());
        }

        let response = unsafe { &*module_response };

        let body = if response.body.is_empty() {
            bytes::Bytes::new()
        } else {
            response.body.clone()
        };

        let mut http_response = http::Response::builder().status(response.status_code);

        let headers_slice = response.headers_slice();
        for kv in headers_slice {
            if let (Ok(name), Ok(value)) = (
                kv.key_str().parse::<http::header::HeaderName>(),
                kv.value_str().parse::<http::header::HeaderValue>(),
            ) {
                http_response = http_response.header(name, value);
            }
        }

        let response = http_response
            .body(body)
            .map_err(|e| ModuleHostError::InternalError(e.to_string()))?;

        unsafe {
            module.free_response(module_response);
        }

        if let Err(e) = session.write_response(response).await {
            tracing::warn!("Failed to write response to session: {}", e);
        }

        Ok(())
    }
}
