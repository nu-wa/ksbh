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

#[cfg(test)]
mod tests {
    use super::*;
    use http::Response;

    #[test]
    fn test_http_response_new() {
        let status = pingora_http::StatusCode::OK;
        let response = Response::new(bytes::Bytes::new());
        let parts = response.into_parts().0;

        let http_response = HttpResponse::new(status, &parts, false);

        assert_eq!(http_response.status_code, 200);
        assert!(!http_response.changed);
    }

    #[test]
    fn test_http_response_with_headers() {
        let status = pingora_http::StatusCode::NOT_FOUND;
        let response = http::Response::builder()
            .header("Content-Type", "application/json")
            .header("X-Custom", "test")
            .body(bytes::Bytes::new())
            .unwrap();
        let (parts, _) = response.into_parts();

        let http_response = HttpResponse::new(status, &parts, true);

        assert_eq!(http_response.status_code, 404);
        assert!(http_response.changed);
        assert_eq!(
            http_response.headers.get("content-type"),
            Some(&crate::KsbhStr::new("application/json"))
        );
        assert_eq!(
            http_response.headers.get("x-custom"),
            Some(&crate::KsbhStr::new("test"))
        );
    }

    #[test]
    fn test_http_response_serialize() {
        let mut headers = ::std::collections::HashMap::new();
        headers.insert(
            crate::KsbhStr::new("Content-Type"),
            crate::KsbhStr::new("text/html"),
        );

        let response = HttpResponse {
            headers,
            status_code: 200,
            changed: false,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("200"));
        assert!(serialized.contains("Content-Type"));
    }

    #[test]
    fn test_http_response_deserialize() {
        let json = r#"{"headers":{"Content-Type":"text/html"},"status_code":200,"changed":false}"#;
        let response: HttpResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.status_code, 200);
        assert!(!response.changed);
        assert_eq!(
            response.headers.get("Content-Type"),
            Some(&crate::KsbhStr::new("text/html"))
        );
    }

    #[test]
    fn test_http_response_debug() {
        let response = HttpResponse {
            headers: ::std::collections::HashMap::new(),
            status_code: 500,
            changed: true,
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("500"));
        assert!(debug_str.contains("changed"));
    }
}
