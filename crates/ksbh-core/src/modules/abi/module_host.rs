/// Errors that can occur when interacting with module host.
#[derive(Debug, PartialEq)]
pub enum ModuleHostError {
    /// Requested module is not loaded
    ModuleNotFound,
    /// Module returned an error
    ModuleError(String),
    /// Internal host error
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

/// Hosts loaded modules and dispatches requests to them.
/// Manages module lifecycle, session storage, and response handling.
pub struct ModuleHost {
    modules: scc::HashMap<
        crate::modules::ModuleConfigurationType,
        super::module_instance::ModuleInstance,
    >,
    cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
    store: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
}

impl ModuleHost {
    pub fn new(
        cookie_settings: ::std::sync::Arc<crate::cookies::CookieSettings>,
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
            cookie_settings,
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

    /// Async FFI call to module request_filter entry point.
    /// Builds module context, invokes the module, and writes response to session.
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

        if !body.is_empty() {
            let has_content_length = http_response
                .headers_ref()
                .is_some_and(|headers| headers.contains_key(http::header::CONTENT_LENGTH));

            if !has_content_length {
                http_response =
                    http_response.header(http::header::CONTENT_LENGTH, body.len().to_string());
            }
        }

        if req_ctx.needs_session_cookie
            && !response_sets_proxy_cookie(&http_response, &self.cookie_settings.name)
        {
            let cookie = crate::cookies::ProxyCookie::new(
                req_ctx.request_info.get_host().unwrap_or_default(),
                uuid::Uuid::from_bytes(req_ctx.session_id_bytes),
            );

            let cookie_header = cookie
                .to_cookie_header(&self.cookie_settings)
                .map_err(|e| ModuleHostError::InternalError(e.to_string()))?;
            let cookie_header = http::HeaderValue::from_str(&cookie_header)
                .map_err(|e| ModuleHostError::InternalError(e.to_string()))?;

            http_response = http_response.header(http::header::SET_COOKIE, cookie_header);
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

fn response_sets_proxy_cookie(response: &http::response::Builder, cookie_name: &str) -> bool {
    let headers = match response.headers_ref() {
        Some(headers) => headers,
        None => return false,
    };

    for header in headers.get_all(http::header::SET_COOKIE) {
        let header_value = match header.to_str() {
            Ok(header_value) => header_value,
            Err(_) => continue,
        };

        let first_segment = match header_value.split(';').next() {
            Some(first_segment) => first_segment,
            None => continue,
        };

        let parsed_cookie_name = match first_segment.split_once('=') {
            Some((parsed_cookie_name, _)) => parsed_cookie_name.trim(),
            None => continue,
        };

        if parsed_cookie_name == cookie_name {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn unit_response_sets_proxy_cookie_malformed_headers_do_not_panic() {
        let malformed = ["", "not-a-cookie", "still-bad;", "a=b;c", "===", ";=;"];
        for value in malformed {
            let builder = http::Response::builder().header(http::header::SET_COOKIE, value);
            let _ = super::response_sets_proxy_cookie(&builder, "ksbh");
        }
    }

    #[test]
    fn response_sets_proxy_cookie_detects_cookie_name_only_at_cookie_key_boundary() {
        let present =
            http::Response::builder().header(http::header::SET_COOKIE, "ksbh=value; Path=/");
        let near_miss =
            http::Response::builder().header(http::header::SET_COOKIE, "ksbh2=value; Path=/");
        let mixed = http::Response::builder()
            .header(http::header::SET_COOKIE, "ksbh2=value; Path=/")
            .header(http::header::SET_COOKIE, "other=value; Path=/");

        assert!(super::response_sets_proxy_cookie(&present, "ksbh"));
        assert!(!super::response_sets_proxy_cookie(&near_miss, "ksbh"));
        assert!(!super::response_sets_proxy_cookie(&mixed, "ksbh"));
    }

    proptest::proptest! {
        #[test]
        fn proptest_response_sets_proxy_cookie_exact_name_match(
            cookie_name in "[a-z]{1,12}",
            other_name in "[a-z]{1,12}",
            cookie_value in "[a-z0-9]{0,16}",
        ) {
            proptest::prop_assume!(cookie_name != other_name);

            let exact_header = ::std::format!("{}={}; Path=/", cookie_name, cookie_value);
            let near_header = ::std::format!("{}2={}; Path=/", cookie_name, cookie_value);
            let other_header = ::std::format!("{}={}; Path=/", other_name, cookie_value);

            let exact_builder = http::Response::builder().header(http::header::SET_COOKIE, exact_header);
            let near_builder = http::Response::builder().header(http::header::SET_COOKIE, near_header);
            let other_builder = http::Response::builder().header(http::header::SET_COOKIE, other_header);

            proptest::prop_assert!(super::response_sets_proxy_cookie(&exact_builder, &cookie_name));
            proptest::prop_assert!(!super::response_sets_proxy_cookie(&near_builder, &cookie_name));
            proptest::prop_assert!(!super::response_sets_proxy_cookie(&other_builder, &cookie_name));
        }
    }
}
