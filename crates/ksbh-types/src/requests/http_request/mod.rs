pub mod error;

// A "parsed" request from a [pingora session](https://docs.rs/pingora-proxy/latest/pingora_proxy/struct.Session.html#method.req_header),
// wich itself has underlying data coming from [`http::request::Parts`](https://docs.rs/http/1.1.0/http/request/struct.Parts.html).
//
// For now we copy the underlying references and do some parsing on it, so it can be used in plugins/modules in the most simplest way.
// Maybe one day I'll use references to avoid over-copying data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HttpRequest {
    pub uri: crate::KsbhStr,
    pub base_url: crate::KsbhStr,
    pub host: crate::KsbhStr,
    pub port: u16,
    pub query: crate::prelude::HttpQuery,
    pub scheme: crate::prelude::HttpScheme,
    pub req_uuid: uuid::Uuid,
    pub method: crate::prelude::HttpMethod,
}

#[derive(Debug)]
pub struct HttpRequestView<'a> {
    pub uri: String,
    pub base_url: String,
    pub host: &'a str,
    pub port: u16,
    pub query: crate::requests::http_query::HttpQueryView<'a>,
    pub req_uuid: uuid::Uuid,
    pub method: crate::requests::http_method::HttpMethodView<'a>,
    pub scheme: crate::requests::http_scheme::HttpScheme,
}

impl HttpRequest {
    pub fn new(
        req_header: &http::request::Parts,
        req_uuid: uuid::Uuid,
        config: &crate::Ports,
    ) -> Result<Self, error::HttpRequestError> {
        let uri = &req_header.uri;
        let query = crate::prelude::HttpQuery::new(req_header)?;
        let mut port = uri.port_u16();

        let mut host = match uri.authority() {
            Some(authority) => authority.to_string(),
            None => match uri.host() {
                Some(host) => host.to_string(),
                None => match req_header.headers.get("Host") {
                    Some(host_header) => match host_header.to_str() {
                        Ok(host_header) => host_header.to_string(),
                        Err(e) => return Err(e.into()),
                    },
                    None => return Err(error::HttpRequestError::InvalidRequest),
                },
            },
        };

        // In case we get an authority and a port; i.e. 'user@password:example.com:8081',
        // should not happen but let's try to handle it.
        if host.contains(":") {
            let authority_split: Vec<&str> = host.split("@").collect();
            let split: Vec<&str> = match authority_split.is_empty() {
                true => host.split(":").collect(),
                false => {
                    let mut index = 0;
                    if authority_split.len() > 1 {
                        index = authority_split.len() - 1;
                    }
                    authority_split[index].split(":").collect()
                }
            };

            if let Some(port_from_req) = split.last() {
                port = port_from_req.parse::<u16>().ok();
            }

            if let Some(host_without_port) = split.first() {
                host = host_without_port.to_string();
            }
        }

        let is_ssl_port = port.map(|p| p == config.https).unwrap_or(false);
        let mut scheme_str = uri.scheme().map(|scheme| scheme.as_str()).unwrap_or("http");

        let is_websocket = req_header
            .headers
            .get("upgrade")
            .map(|v| v.to_str().unwrap_or("").to_lowercase() == "websocket")
            .unwrap_or(false);

        if is_ssl_port {
            scheme_str = if is_websocket { "wss" } else { "https" };
        } else {
            if let Some(forwarded_proto) = req_header.headers.get("x-forwarded-proto") {
                scheme_str = forwarded_proto.to_str()?;
            }
            if is_websocket && scheme_str == "http" {
                scheme_str = "ws";
            }
        }

        let is_secure_proto = scheme_str == "https" || scheme_str == "wss";
        let target_config_port = if is_secure_proto {
            config.https
        } else {
            config.http
        };
        let effective_port = port.unwrap_or(target_config_port);

        let base_url = format!("{}://{}{}", scheme_str, host, {
            let is_standard = (is_secure_proto && effective_port == 443)
                || (!is_secure_proto && effective_port == 80);
            if !is_standard {
                format!(":{}", effective_port)
            } else {
                String::new()
            }
        });

        let full_uri = format!("{}{}", base_url, query);

        let final_scheme = if is_secure_proto {
            crate::prelude::HttpScheme(http::uri::Scheme::HTTPS)
        } else {
            crate::prelude::HttpScheme(http::uri::Scheme::HTTP)
        };

        Ok(Self {
            uri: crate::KsbhStr::new(full_uri),
            query,
            host: crate::KsbhStr::new(host),
            scheme: final_scheme,
            port: effective_port,
            req_uuid,
            base_url: crate::KsbhStr::new(base_url),
            method: crate::prelude::HttpMethod(req_header.method.to_owned()),
        })
    }

    pub fn to_owned(&self) -> Self {
        Self {
            uri: self.uri.clone(),
            host: self.host.clone(),
            port: self.port,
            scheme: self.scheme.clone(),
            query: self.query.to_owned(),
            req_uuid: self.req_uuid,
            base_url: self.base_url.clone(),
            method: self.method.to_owned(),
        }
    }
}

#[cfg(feature = "test-util")]
impl HttpRequest {
    pub fn t_create(host: &str, path: Option<&[u8]>, method: Option<&str>) -> HttpRequest {
        let req_uuid = uuid::Uuid::new_v4();

        let mut headers = pingora_http::RequestHeader::build_no_case(
            method.unwrap_or("GET"),
            path.unwrap_or(b"/"),
            None,
        )
        .unwrap();

        headers.insert_header(http::header::HOST, host).unwrap();

        HttpRequest::new(
            &headers,
            req_uuid,
            &crate::Ports {
                https: 443,
                http: 80,
            },
        )
        .unwrap()
    }
}

impl<'a> HttpRequestView<'a> {
    pub fn new(
        req_header: &'a http::request::Parts,
        req_uuid: uuid::Uuid,
        config: &crate::Ports,
    ) -> Result<Self, error::HttpRequestError> {
        let uri = &req_header.uri;
        let query = crate::requests::http_query::HttpQueryView::new(req_header)?;
        let mut port = uri.port_u16();

        let mut host = match uri.authority() {
            Some(authority) => authority.as_str(),
            None => match uri.host() {
                Some(host) => host,
                None => match req_header.headers.get("Host") {
                    Some(host_header) => host_header.to_str()?,
                    None => return Err(error::HttpRequestError::InvalidRequest),
                },
            },
        };

        // In case we get an authority and a port; i.e. 'user@password:example.com:8081',
        // should not happen but let's try to handle it.
        if host.contains(":") {
            let authority_split: Vec<&str> = host.split("@").collect();
            let split: Vec<&str> = match authority_split.is_empty() {
                true => host.split(":").collect(),
                false => {
                    let mut index = 0;
                    if authority_split.len() > 1 {
                        index = authority_split.len() - 1;
                    }
                    authority_split[index].split(":").collect()
                }
            };

            if let Some(port_from_req) = split.last() {
                port = port_from_req.parse::<u16>().ok();
            }

            if let Some(host_without_port) = split.first() {
                host = host_without_port;
            }
        }

        let is_ssl_port = port.map(|p| p == config.https).unwrap_or(false);
        let mut scheme_str = uri.scheme().map(|scheme| scheme.as_str()).unwrap_or("http");

        let is_websocket = req_header
            .headers
            .get("upgrade")
            .map(|v| v.to_str().unwrap_or("").to_lowercase() == "websocket")
            .unwrap_or(false);

        if is_ssl_port {
            scheme_str = if is_websocket { "wss" } else { "https" };
        } else {
            if let Some(forwarded_proto) = req_header.headers.get("x-forwarded-proto") {
                scheme_str = forwarded_proto.to_str()?;
            }
            if is_websocket && scheme_str == "http" {
                scheme_str = "ws";
            }
        }

        let is_secure_proto = scheme_str == "https" || scheme_str == "wss";
        let target_config_port = if is_secure_proto {
            config.https
        } else {
            config.http
        };
        let effective_port = port.unwrap_or(target_config_port);

        let base_url = format!("{}://{}{}", scheme_str, host, {
            let is_standard = (is_secure_proto && effective_port == 443)
                || (!is_secure_proto && effective_port == 80);
            if !is_standard {
                format!(":{}", effective_port)
            } else {
                String::new()
            }
        });

        let full_uri = format!("{}{}", base_url, query);

        let final_scheme = if is_secure_proto {
            crate::prelude::HttpScheme(http::uri::Scheme::HTTPS)
        } else {
            crate::prelude::HttpScheme(http::uri::Scheme::HTTP)
        };

        Ok(Self {
            uri: full_uri,
            query,
            host,
            scheme: final_scheme,
            port: effective_port,
            req_uuid,
            base_url,
            method: crate::requests::http_method::HttpMethodView(req_header.method.as_str()),
        })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_parsing() {
        use super::HttpRequest;
        use crate::Ports;
        use crate::prelude::HttpScheme;
        let req_id = uuid::Uuid::new_v4();

        let config = Ports {
            https: 443,
            http: 80,
        };
        let request_header = pingora_http::RequestHeader::build_no_case("GET", b"", None);

        assert!(request_header.is_ok());
        let mut request_header = request_header.unwrap();
        request_header.set_uri("http://example.com".parse::<http::uri::Uri>().unwrap());

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        request_header.set_uri("example.com".parse::<http::uri::Uri>().unwrap());

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("http://example.com", &parsed_request.uri);
        assert_eq!(80, parsed_request.port);
        request_header.set_uri("example.com:8081".parse::<http::uri::Uri>().unwrap());

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("http://example.com:8081", &parsed_request.uri);
        assert_eq!(8081, parsed_request.port);

        request_header.set_uri("example.com:80".parse::<http::Uri>().unwrap());
        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!(HttpScheme(http::uri::Scheme::HTTP), parsed_request.scheme);

        request_header.set_uri(
            "http://user:password@example.com:80"
                .parse::<http::Uri>()
                .unwrap(),
        );
        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("example.com", &parsed_request.host);

        request_header.set_uri(
            "https://user:password@example.com"
                .parse::<http::Uri>()
                .unwrap(),
        );

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);

        request_header.set_uri(
            "https://user:password@example.com:8080/path?foo&bar=test"
                .parse::<http::Uri>()
                .unwrap(),
        );

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("/path", &parsed_request.query.path);
        assert_eq!(8080_u16, parsed_request.port);
        assert_eq!(
            parsed_request.query.get_param("foo"),
            Some(&crate::KsbhStr::new(""))
        );
        assert_eq!(
            parsed_request.query.get_param("bar"),
            Some(&crate::KsbhStr::new("test"))
        );
        assert_eq!(HttpScheme(http::uri::Scheme::HTTPS), parsed_request.scheme);

        request_header.set_uri(
            "https://example.com:8081/path?foo&bar=test"
                .parse::<http::Uri>()
                .unwrap(),
        );

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("/path", &parsed_request.query.path);
        assert_eq!(8081_u16, parsed_request.port);
        assert_eq!(
            parsed_request.query.get_param("foo"),
            Some(&crate::KsbhStr::new(""))
        );
        assert_eq!(
            parsed_request.query.get_param("bar"),
            Some(&crate::KsbhStr::new("test"))
        );
        assert_eq!(HttpScheme(http::uri::Scheme::HTTPS), parsed_request.scheme);

        let config = Ports {
            https: 443,
            http: 80,
        };

        request_header.set_uri(
            "http://example.com/path?foo&bar=test"
                .parse::<http::Uri>()
                .unwrap(),
        );

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("/path", &parsed_request.query.path);
        assert_eq!(80_u16, parsed_request.port);
        assert_eq!(HttpScheme(http::uri::Scheme::HTTP), parsed_request.scheme);

        request_header.set_uri(
            "https://example.com/path?foo&bar=test"
                .parse::<http::Uri>()
                .unwrap(),
        );

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();

        assert_eq!("example.com", &parsed_request.host);
        assert_eq!("/path", &parsed_request.query.path);
        assert_eq!(443, parsed_request.port);
        assert_eq!(HttpScheme(http::uri::Scheme::HTTPS), parsed_request.scheme);
        assert_eq!(
            "https://example.com/path?foo&bar=test",
            parsed_request.uri.as_str()
        );

        let config = Ports {
            https: 443,
            http: 80,
        };
        let request_header = pingora_http::RequestHeader::build_no_case("GET", b"", None);

        assert!(request_header.is_ok());
        let mut request_header = request_header.unwrap();
        request_header.set_uri("http://example.com:8080".parse::<http::uri::Uri>().unwrap());

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();
        assert_eq!(parsed_request.uri, "http://example.com:8080/");

        request_header.set_uri("http://example.com".parse::<http::uri::Uri>().unwrap());

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();
        assert_eq!(parsed_request.uri, "http://example.com:8080/");

        request_header.set_uri(
            "http://example.com/testing"
                .parse::<http::uri::Uri>()
                .unwrap(),
        );

        let parsed_request = HttpRequest::new(&request_header, req_id, &config);

        assert!(parsed_request.is_ok());
        let parsed_request = parsed_request.unwrap();
        assert_eq!(parsed_request.base_url, "http://example.com:8080");
        assert_eq!(parsed_request.uri, "http://example.com:8080/testing");
    }
}
