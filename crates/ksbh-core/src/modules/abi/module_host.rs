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
    session_store: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    #[allow(dead_code)]
    metrics_store: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::proxy::PartialClientInformation,
            crate::metrics::Hits,
        >,
    >,
}

impl ModuleHost {
    pub fn new(
        sessions: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
        metrics: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::proxy::PartialClientInformation,
                crate::metrics::Hits,
            >,
        >,
    ) -> Self {
        super::host_functions::set_module_sessions(sessions.clone());
        super::host_functions::set_module_metrics(metrics.clone());
        Self {
            modules: scc::HashMap::new(),
            session_store: sessions,
            metrics_store: metrics,
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
        &self.session_store
    }

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

    pub async fn call_module<'req>(
        &self,
        module_type: &crate::modules::ModuleConfigurationType,
        config_kv_slice: &[super::ModuleKvSlice],
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        request_view: &'req ksbh_types::requests::http_request::HttpRequestView<'req>,
        body: Option<&[u8]>,
        module_name: &'req str,
    ) -> Result<(), ModuleHostError> {
        let module = match self.modules.get_sync(module_type) {
            Some(m) => m,
            None => return Err(ModuleHostError::ModuleNotFound),
        };

        let mut headers_vec: Vec<super::ModuleKvSlice> = Vec::with_capacity(32);
        for (name, value) in session.headers().headers.iter() {
            let name_str = name.as_str();
            let value_bytes = value.as_bytes();

            let key = name_str.as_ptr();
            let key_len = name_str.len();
            let value = value_bytes.as_ptr();
            let value_len = value_bytes.len();

            headers_vec.push(super::ModuleKvSlice {
                key,
                key_len,
                value,
                value_len,
            });
        }
        let headers = headers_vec;

        let mut query_params_vec: Vec<super::ModuleKvSlice> = Vec::with_capacity(16);

        for (k, v) in request_view.query.params.iter() {
            let key = k.as_ptr();
            let key_len = k.len();
            let value = v.as_ptr();
            let value_len = v.len();

            query_params_vec.push(super::ModuleKvSlice {
                key,
                key_len,
                value,
                value_len,
            });
        }
        let query_params = query_params_vec;

        let request_info =
            super::module_request_info::RequestInfo::new(request_view, &query_params);

        let session_id = request_view.req_uuid.into_bytes();

        let client_ip = crate::utils::get_client_ip_from_session(session)
            .map(|ip| ip.to_string())
            .unwrap_or_default();

        let user_agent = session
            .headers()
            .headers
            .get(http::header::USER_AGENT)
            .and_then(|ua| ua.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let cookie_header = session
            .headers()
            .headers
            .get(http::header::COOKIE)
            .and_then(|c| c.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let mut ctx = super::ModuleContext {
            config: config_kv_slice,
            headers: &headers,
            req_info: &request_info,
            body: body.unwrap_or(&[]),
            log_fn: super::host_functions::host_log_callback,
            session_id,
            session_get_fn: super::host_functions::host_session_get,
            session_set_fn: super::host_functions::host_session_set,
            session_set_with_ttl_fn: super::host_functions::host_session_set_with_ttl,
            session_free_fn: super::host_functions::host_session_free,
            mod_name: module_name,
            client_ip: super::ModuleBuffer::from_ref(&client_ip),
            user_agent: super::ModuleBuffer::from_ref(&user_agent),
            cookie_header: super::ModuleBuffer::from_ref(&cookie_header),
            metrics_increment_good_fn: super::host_functions::host_metrics_increment_good,
            metrics_get_hits_fn: super::host_functions::host_metrics_get_hits,
        };

        let module_response = module.call_request_filter(&mut ctx);

        if module_response.is_null() {
            return Ok(());
        }

        let response = unsafe { &*module_response };

        let body = if response.body.is_null() || response.body_size == 0 {
            bytes::Bytes::new()
        } else {
            bytes::Bytes::copy_from_slice(unsafe {
                std::slice::from_raw_parts(response.body, response.body_size)
            })
        };

        let mut http_response = http::Response::builder().status(response.status_code);

        if !response.headers.is_null() && response.headers_size > 0 {
            let headers_data =
                unsafe { std::slice::from_raw_parts(response.headers, response.headers_size) };

            let mut offset = 0;
            while offset < headers_data.len() {
                let key_len = u32::from_le_bytes([
                    headers_data[offset],
                    headers_data[offset + 1],
                    headers_data[offset + 2],
                    headers_data[offset + 3],
                ]) as usize;
                offset += 4;

                let key = std::str::from_utf8(&headers_data[offset..offset + key_len]).ok();
                offset += key_len;

                let value_len = u32::from_le_bytes([
                    headers_data[offset],
                    headers_data[offset + 1],
                    headers_data[offset + 2],
                    headers_data[offset + 3],
                ]) as usize;
                offset += 4;

                let value = std::str::from_utf8(&headers_data[offset..offset + value_len]).ok();
                offset += value_len;

                if let (Some(k), Some(v)) = (key, value) {
                    http_response = http_response.header(k, v);
                }
            }
        }

        let response = http_response
            .body(body)
            .map_err(|e| ModuleHostError::InternalError(e.to_string()))?;

        unsafe {
            module.free_response(module_response);
        }

        let _ = session.write_response(response).await;

        Ok(())
    }
}
