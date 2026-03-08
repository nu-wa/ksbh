use ::std::str::FromStr;
#[cfg(feature = "test-util")]
use ::std::sync::Mutex;

use crate::providers::proxy::{ProxyProviderError, ProxyProviderSession};

#[derive(Debug)]
pub struct MockProxyProviderSession {
    request_headers: http::request::Parts,
    request_uri: Option<http::Uri>,
    client_addr: Option<::std::net::IpAddr>,
    request_body: Option<bytes::Bytes>,
    response_written: Option<http::Response<bytes::Bytes>>,
    write_response_error: Option<ProxyProviderError>,
    read_request_body_error: Option<ProxyProviderError>,
    last_written_response: Mutex<Option<http::Response<bytes::Bytes>>>,
}

impl Clone for MockProxyProviderSession {
    fn clone(&self) -> Self {
        Self {
            request_headers: self.request_headers.clone(),
            request_uri: self.request_uri.clone(),
            client_addr: self.client_addr,
            request_body: self.request_body.clone(),
            response_written: self.response_written.clone(),
            write_response_error: self.write_response_error.clone(),
            read_request_body_error: self.read_request_body_error.clone(),
            last_written_response: Mutex::new(None),
        }
    }
}

pub struct MockBuilder {
    session: MockProxyProviderSession,
}

impl MockBuilder {
    pub fn new() -> Self {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::HOST,
            http::HeaderValue::from_static("localhost"),
        );

        let request = http::Request::builder()
            .method(http::Method::GET)
            .uri("http://localhost/")
            .body(())
            .expect("Failed to build request");

        let (mut request_headers, _) = request.into_parts();
        request_headers.headers = headers;

        Self {
            session: MockProxyProviderSession {
                request_headers,
                request_uri: None,
                client_addr: None,
                request_body: None,
                response_written: None,
                write_response_error: None,
                read_request_body_error: None,
                last_written_response: Mutex::new(None),
            },
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        let header_name =
            http::HeaderName::from_bytes(name.as_bytes()).expect("Invalid header name");
        let header_value = http::HeaderValue::from_str(value).expect("Invalid header value");
        self.session
            .request_headers
            .headers
            .insert(header_name, header_value);
        self
    }

    pub fn path(mut self, path: &str) -> Self {
        let new_uri = ::std::format!("http://localhost{}", path);
        self.session.request_headers.uri = http::Uri::from_str(&new_uri).expect("Invalid URI");
        self
    }

    pub fn method(mut self, method: http::Method) -> Self {
        self.session.request_headers.method = method;
        self
    }

    pub fn uri(mut self, uri: &str) -> Self {
        self.session.request_headers.uri = http::Uri::from_str(uri).expect("Invalid URI");
        self
    }

    pub fn client_addr(mut self, addr: ::std::net::IpAddr) -> Self {
        self.session.client_addr = Some(addr);
        self
    }

    pub fn request_body(mut self, body: bytes::Bytes) -> Self {
        self.session.request_body = Some(body);
        self
    }

    pub fn response_written(mut self, response: http::Response<bytes::Bytes>) -> Self {
        self.session.response_written = Some(response);
        self
    }

    pub fn write_response_error(mut self, error: ProxyProviderError) -> Self {
        self.session.write_response_error = Some(error);
        self
    }

    pub fn read_request_body_error(mut self, error: ProxyProviderError) -> Self {
        self.session.read_request_body_error = Some(error);
        self
    }

    pub fn build(self) -> MockProxyProviderSession {
        self.session
    }
}

impl Default for MockBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProxyProviderSession {
    pub fn builder() -> MockBuilder {
        MockBuilder::new()
    }

    pub fn assert_header_present(&self, name: &str) {
        let header_name =
            http::HeaderName::from_bytes(name.as_bytes()).expect("Invalid header name");
        assert!(
            self.request_headers.headers.contains_key(header_name),
            "Header '{}' not found",
            name
        );
    }

    pub fn assert_header_value(&self, name: &str, expected: &str) {
        let header_name =
            http::HeaderName::from_bytes(name.as_bytes()).expect("Invalid header name");
        let actual = self
            .request_headers
            .headers
            .get(header_name)
            .unwrap_or_else(|| panic!("Header '{}' not found", name));
        let actual_str = actual.to_str().expect("Invalid header value");
        assert_eq!(
            actual_str, expected,
            "Header '{}' expected '{}' but got '{}'",
            name, expected, actual_str
        );
    }

    pub fn assert_request_uri(&self, expected: &str) {
        let uri = self.request_uri.as_ref().expect("Request URI not set");
        let uri_str = uri.to_string();
        assert_eq!(
            uri_str, expected,
            "Request URI expected '{}' but got '{}'",
            expected, uri_str
        );
    }

    pub fn assert_written_status(&self, status: http::StatusCode) {
        let guard = self.last_written_response.lock().expect("Lock poisoned");
        let response = guard.as_ref().expect("No response written");
        assert_eq!(
            response.status(),
            status,
            "Written response status expected {} but got {}",
            status,
            response.status()
        );
    }

    pub fn take_written_response(&self) -> Option<http::Response<bytes::Bytes>> {
        let mut guard = self.last_written_response.lock().expect("Lock poisoned");
        guard.take()
    }
}

#[async_trait::async_trait]
impl ProxyProviderSession for MockProxyProviderSession {
    fn headers(&self) -> http::request::Parts {
        self.request_headers.clone()
    }

    fn get_header(&self, header: http::HeaderName) -> Option<&http::header::HeaderValue> {
        self.request_headers.headers.get(header)
    }

    fn set_request_uri(&mut self, uri: http::Uri) {
        self.request_uri = Some(uri);
    }

    fn response_written(&self) -> Option<http::Response<bytes::Bytes>> {
        self.response_written.clone()
    }

    fn client_addr(&self) -> Option<::std::net::IpAddr> {
        self.client_addr
    }

    async fn write_response(
        &mut self,
        response: http::Response<bytes::Bytes>,
    ) -> Result<(), ProxyProviderError> {
        if let Some(ref err) = self.write_response_error {
            return Err(err.clone());
        }
        let mut guard = self.last_written_response.lock().expect("Lock poisoned");
        *guard = Some(response);
        Ok(())
    }

    async fn read_request_body(&mut self) -> Result<Option<bytes::Bytes>, ProxyProviderError> {
        if let Some(ref err) = self.read_request_body_error {
            return Err(err.clone());
        }
        Ok(self.request_body.clone())
    }
}
