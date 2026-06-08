use ::std::collections::HashMap;
use http::HeaderMap;

/// Module Context passed from Host to module, only valid for `'ctx`, hence why we try to avoid
/// allocations and just use views.
pub struct ModuleContext<'ctx> {
    pub host_ctx: u64,
    pub config: HashMap<&'ctx str, &'ctx str>,
    pub headers: HeaderMap,
    pub request_info: RequestInfo<'ctx>,
    pub body: &'ctx [u8],
    pub cookie_header: &'ctx str,
    pub reputation_key: &'ctx [u8],
    pub internal_path: &'ctx str,
    pub session_id: &'ctx [u8; 16],
    logger: crate::logger::Logger,
    session: crate::session::SessionHandle,
    reputation: crate::metrics::ReputationHandle,
    host_signal_get: ksbh_modules_abi::functions::HostFnSignalGet,
}

/// Parsed HTTP request information.
///
/// Provides convenient access to request components extracted from the URI.
pub struct RequestInfo<'ctx> {
    /// Full request URI (e.g., `https://example.com/path?query=value`).
    pub uri: &'ctx str,
    /// Host header value (e.g., `example.com`).
    pub host: &'ctx str,
    /// HTTP method (GET, POST, etc.).
    pub method: &'ctx str,
    /// Request path (e.g., `/api/users`).
    pub path: &'ctx str,
    /// Query string parameters parsed into a map.
    pub query_params: HashMap<&'ctx str, &'ctx str>,
    /// Request scheme (http or https).
    pub scheme: &'ctx str,
    /// Request port number.
    pub port: u16,
    /// Whether this request is a websocket handshake.
    pub is_websocket_handshake: bool,
}

impl<'ctx> TryFrom<&'ctx ksbh_modules_abi::types::ModuleContext> for ModuleContext<'ctx> {
    type Error = anyhow::Error;

    fn try_from(value: &'ctx ksbh_modules_abi::types::ModuleContext) -> Result<Self, Self::Error> {
        use crate::utils::*;

        Ok(Self {
            host_ctx: value.host_ctx.inner,
            config: abi_string_to_sdk_hashmap(&value.config)?,
            // For now this allocates, I'm fine with it.
            headers: abi_headers_to_sdk(&value.headers)?,
            request_info: unsafe {
                value
                    .request_info
                    .as_ref()
                    .ok_or(anyhow::anyhow!("ModuleCtx did not have any RequestInfo"))?
                    .try_into()?
            },
            body: abi_bytes_slice_to_sdk(&value.body)?,
            cookie_header: abi_string_to_sdk(&value.cookie_header)?,
            reputation_key: abi_bytes_slice_to_sdk(&value.reputation_key)?,
            internal_path: abi_string_to_sdk(&value.internal_path)?,
            session_id: &value.session_id.inner,
            logger: crate::logger::Logger::new(value.host_ctx.inner, value.h_log_fn),
            session: crate::session::SessionHandle::from_ffi(
                value.host_ctx.inner,
                value.session_id.inner,
                value.h_session_get_fn,
                value.h_session_set_fn,
                value.h_free_bytes,
                value.h_session_shared_get_fn,
                value.h_session_shared_set_fn,
            ),
            reputation: crate::metrics::ReputationHandle::from_ffi(
                value.host_ctx.inner,
                value.h_reputation_good_boy_fn,
                value.h_reputation_get_score_fn,
            ),
            host_signal_get: value.h_signal_get_fn,
        })
    }
}

impl<'ctx> TryFrom<&'ctx ksbh_modules_abi::prelude::RequestInfo> for RequestInfo<'ctx> {
    type Error = anyhow::Error;

    fn try_from(value: &'ctx ksbh_modules_abi::prelude::RequestInfo) -> Result<Self, Self::Error> {
        use crate::utils::*;
        Ok(Self {
            uri: abi_string_to_sdk(&value.uri)?,
            is_websocket_handshake: value.is_ws_handshake != 0,
            host: abi_string_to_sdk(&value.host)?,
            method: abi_string_to_sdk(&value.method)?,
            path: abi_string_to_sdk(&value.path)?,
            query_params: abi_string_to_sdk_hashmap(&value.query_params)?,
            scheme: abi_string_to_sdk(&value.scheme)?,
            port: value.port,
        })
    }
}

impl<'ctx> ModuleContext<'ctx> {
    pub fn require_config(&self, key: &str) -> Result<&'ctx str, crate::ModuleError> {
        self.config
            .get(key)
            .copied()
            .ok_or_else(|| crate::ModuleError::missing_config(key))
    }

    pub fn body_utf8(&self) -> Result<&'ctx str, crate::ModuleError> {
        Ok(::std::str::from_utf8(self.body)?)
    }

    pub fn session_get_parse<T>(&self, key: &str) -> Result<Option<T>, crate::ModuleError>
    where
        T: ::std::str::FromStr,
        T::Err: ::std::fmt::Display,
    {
        let Some(raw) = self.session.get(key)? else {
            return Ok(None);
        };

        let value = ::std::str::from_utf8(&raw)?;
        let parsed = value.parse::<T>().map_err(|error| crate::ModuleError::Abi {
            message: format!("failed to parse session key `{key}`: {error}"),
        })?;

        Ok(Some(parsed))
    }

    pub fn log_error(&self, msg: &str) {
        self.logger.error(msg);
    }

    pub fn log_warn(&self, msg: &str) {
        self.logger.warn(msg);
    }

    pub fn log_debug(&self, msg: &str) {
        self.logger.debug(msg);
    }

    pub fn log_info(&self, msg: &str) {
        self.logger.info(msg);
    }

    pub fn session_get(&self, key: &str) -> Result<Option<Vec<u8>>, crate::ModuleError> {
        self.session.get(key)
    }

    pub fn session_set(
        &self,
        key: &str,
        data: &[u8],
        ttl: Option<u64>,
    ) -> Result<(), crate::ModuleError> {
        self.session.set(key, data, ttl)
    }

    pub fn session_get_shared(&self, key: &str) -> Result<Option<Vec<u8>>, crate::ModuleError> {
        self.session.get_shared(key)
    }

    pub fn session_set_shared(
        &self,
        key: &str,
        data: &[u8],
        ttl: Option<u64>,
    ) -> Result<(), crate::ModuleError> {
        self.session.set_shared(key, data, ttl)
    }

    pub fn reputation_good_boy(&self) -> Result<(), crate::ModuleError> {
        self.reputation.reputation_good_boy()
    }

    pub fn reputation_score(&self) -> Result<Option<u64>, crate::ModuleError> {
        self.reputation.reputation_score()
    }

    pub fn host_signal(
        &self,
        kind: crate::HostSignalKind,
    ) -> Result<u64, crate::ModuleError> {
        unsafe {
            let mut result = 0_u64;

            match (self.host_signal_get)(
                ksbh_modules_abi::types::KSBHHostCtxHandle {
                    inner: self.host_ctx,
                },
                kind,
                &mut result,
            ) {
                ksbh_modules_abi::types::KSBHHostFnReturn::Success => Ok(result),
                ksbh_modules_abi::types::KSBHHostFnReturn::NotFound => Err(
                    crate::ModuleError::Host {
                        message: "host signal not found".to_string(),
                    },
                ),
                ksbh_modules_abi::types::KSBHHostFnReturn::BadArgument => Err(
                    crate::ModuleError::Host {
                        message: "host signal rejected: bad argument".to_string(),
                    },
                ),
                ksbh_modules_abi::types::KSBHHostFnReturn::HostFailure => Err(
                    crate::ModuleError::Host {
                        message: "host signal failed".to_string(),
                    },
                ),
            }
        }
    }
}
