#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HttpResponse {
    pub headers: ::std::collections::HashMap<crate::KsbhStr, crate::KsbhStr>,
    pub status_code: u16,
    pub changed: bool,
}

impl HttpResponse {
    // TODO: implement error handling
    pub fn new(
        status_code: pingora_http::StatusCode,
        req_parts: &http::response::Parts,
        changed: bool,
    ) -> Self {
        Self {
            changed,
            status_code: status_code.as_u16(),
            headers: {
                let mut headers = ::std::collections::HashMap::new();
                for (k, v) in req_parts.headers.iter() {
                    if let Ok(header_value_str) = v.to_str() {
                        headers.insert(
                            crate::KsbhStr::new(k),
                            crate::KsbhStr::new(header_value_str),
                        );
                    }
                }

                headers
            },
        }
    }
}
