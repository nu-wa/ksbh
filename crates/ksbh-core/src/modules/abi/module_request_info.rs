#[repr(C)]
pub struct RequestInfo {
    pub uri: super::ModuleBuffer,
    pub host: super::ModuleBuffer,
    pub method: super::ModuleBuffer,
    pub path: super::ModuleBuffer,
    pub query_params: QueryParams,
    pub scheme: super::ModuleBuffer,
    pub port: u16,
}

#[repr(C)]
#[derive(Clone)]
pub struct QueryParams {
    pub params: Vec<super::ModuleKvSlice>,
}

impl QueryParams {
    pub fn new(params: &[super::ModuleKvSlice]) -> Self {
        Self {
            params: params.to_vec(),
        }
    }

    pub fn as_slice(&self) -> &[super::ModuleKvSlice] {
        &self.params
    }
}

#[repr(C)]
pub struct IpAddress {
    pub data: *const u8,
    pub len: usize,
    pub is_ipv6: bool,
}

impl IpAddress {
    pub fn new_v4(ptr: *const u8, len: usize) -> Self {
        Self {
            data: ptr,
            len,
            is_ipv6: false,
        }
    }

    pub fn new_v6(ptr: *const u8, len: usize) -> Self {
        Self {
            data: ptr,
            len,
            is_ipv6: true,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { ::std::slice::from_raw_parts(self.data, self.len) }
    }
}

#[repr(C)]
pub struct PublicConfig {
    pub http_port: u16,
    pub https_port: u16,
}

impl RequestInfo {
    pub fn new(
        request_info: &ksbh_types::requests::http_request::HttpRequestView,
        query_params_data: &[super::ModuleKvSlice],
    ) -> Self {
        Self {
            uri: super::ModuleBuffer::from_ref(&request_info.uri),
            host: super::ModuleBuffer::from_ref(request_info.host),
            method: super::ModuleBuffer::from_ref(request_info.method.0),
            path: super::ModuleBuffer::from_ref(request_info.query.path),
            query_params: QueryParams::new(query_params_data),
            scheme: super::ModuleBuffer::from_ref(request_info.scheme.0.as_str()),
            port: request_info.port,
        }
    }

    /// Create from owned HttpRequest (not HttpRequestView).
    pub fn new_owned(
        request: &ksbh_types::requests::http_request::HttpRequest,
        query_params_data: &[super::ModuleKvSlice],
    ) -> Self {
        Self {
            uri: super::ModuleBuffer::from_ref(&request.uri),
            host: super::ModuleBuffer::from_ref(request.host.as_str()),
            method: super::ModuleBuffer::from_ref(request.method.0.as_str()),
            path: super::ModuleBuffer::from_ref(request.query.path.as_str()),
            query_params: QueryParams::new(query_params_data),
            scheme: super::ModuleBuffer::from_ref(request.scheme.0.as_str()),
            port: request.port,
        }
    }

    pub fn get_uri(&self) -> Option<&str> {
        self.uri.as_str()
    }

    pub fn get_scheme(&self) -> Option<&str> {
        self.scheme.as_str()
    }

    pub fn get_host(&self) -> Option<&str> {
        self.host.as_str()
    }

    pub fn get_method(&self) -> Option<&str> {
        self.method.as_str()
    }

    pub fn get_path(&self) -> Option<&str> {
        self.path.as_str()
    }

    pub fn get_query_params(&self) -> &[super::ModuleKvSlice] {
        self.query_params.as_slice()
    }

    pub fn get_query_param(&self, key: &str) -> Option<&str> {
        for entry in &self.query_params.params {
            if entry.key.as_ref() == key.as_bytes() {
                return ::std::str::from_utf8(&entry.value).ok();
            }
        }
        None
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }
}
