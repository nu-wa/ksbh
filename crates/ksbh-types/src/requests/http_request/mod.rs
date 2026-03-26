pub mod error;

fn header_has_token(
    headers: &http::HeaderMap,
    name: impl http::header::AsHeaderName,
    token: &str,
) -> bool {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(token))
        })
        .unwrap_or(false)
}

fn first_header_token(
    headers: &http::HeaderMap,
    name: impl http::header::AsHeaderName,
) -> Result<Option<String>, http::header::ToStrError> {
    Ok(headers
        .get(name)
        .map(|value| value.to_str())
        .transpose()?
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty()))
}

fn forwarded_port(headers: &http::HeaderMap) -> Result<Option<u16>, http::header::ToStrError> {
    Ok(
        first_header_token(headers, "x-forwarded-port")?
            .and_then(|value| value.parse::<u16>().ok()),
    )
}

fn is_websocket_upgrade(headers: &http::HeaderMap) -> bool {
    if !header_has_token(headers, http::header::UPGRADE, "websocket") {
        return false;
    }

    header_has_token(headers, http::header::CONNECTION, "upgrade")
        || headers.contains_key("Sec-WebSocket-Key")
}

fn effective_request_scheme(
    req_header: &http::request::Parts,
    downstream_tls: bool,
    trust_forwarded_headers: bool,
    port: Option<u16>,
) -> Result<String, error::HttpRequestError> {
    let mut scheme = req_header
        .uri
        .scheme()
        .map(|value| value.as_str().to_ascii_lowercase())
        .unwrap_or_else(|| "http".to_string());
    let websocket_upgrade = is_websocket_upgrade(&req_header.headers);

    if downstream_tls {
        return Ok(if websocket_upgrade {
            "wss".to_string()
        } else {
            "https".to_string()
        });
    }

    if trust_forwarded_headers
        && let Some(forwarded_proto) = first_header_token(&req_header.headers, "x-forwarded-proto")?
    {
        scheme = forwarded_proto;
    }

    if websocket_upgrade {
        let tls_forwarded =
            trust_forwarded_headers && forwarded_port(&req_header.headers)? == Some(443);
        let is_secure_scheme =
            scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("wss");
        if is_secure_scheme || port == Some(443) || tls_forwarded {
            scheme = "wss".to_string();
        } else {
            scheme = "ws".to_string();
        }
    } else if scheme.eq_ignore_ascii_case("wss") {
        scheme = "https".to_string();
    } else if scheme.eq_ignore_ascii_case("ws") {
        scheme = "http".to_string();
    }

    Ok(scheme)
}

/// A "parsed" HTTP request with owned data for use in plugins/modules.
///
/// Parsed from a [pingora session](https://docs.rs/pingora-proxy/latest/pingora_proxy/struct.Session.html#method.req_header),
/// which itself has underlying data coming from [`http::request::Parts`](https://docs.rs/http/1.1.0/http/request/struct.Parts.html).
///
/// All string data is copied into owned [`KsbhStr`](crate::KsbhStr) for FFI compatibility.
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

/// A borrowed view of an HTTP request for non-owning contexts.
///
/// Contains borrowed string references (`&'a str`) instead of owned data,
/// useful for in-process request processing without FFI boundaries.
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
        downstream_tls: bool,
        trust_forwarded_headers: bool,
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

        let scheme_string = effective_request_scheme(
            req_header,
            downstream_tls || port.map(|p| p == config.https).unwrap_or(false),
            trust_forwarded_headers,
            port,
        )?;
        let scheme_str = scheme_string.as_str();

        let is_secure_proto =
            scheme_str.eq_ignore_ascii_case("https") || scheme_str.eq_ignore_ascii_case("wss");
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
            false,
            false,
        )
        .unwrap()
    }
}

impl<'a> HttpRequestView<'a> {
    pub fn new(
        req_header: &'a http::request::Parts,
        req_uuid: uuid::Uuid,
        config: &crate::Ports,
        downstream_tls: bool,
        trust_forwarded_headers: bool,
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

        let scheme_string = effective_request_scheme(
            req_header,
            downstream_tls || port.map(|p| p == config.https).unwrap_or(false),
            trust_forwarded_headers,
            port,
        )?;
        let scheme_str = scheme_string.as_str();

        let is_secure_proto =
            scheme_str.eq_ignore_ascii_case("https") || scheme_str.eq_ignore_ascii_case("wss");
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
    fn build_request_parts(path: &str, extra_headers: &[(&str, &str)]) -> http::request::Parts {
        let mut builder = http::Request::builder()
            .method(http::Method::GET)
            .uri(path)
            .header(http::header::HOST, "example.test");
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }
        let request = builder.body(()).expect("build request");
        let (parts, _) = request.into_parts();
        parts
    }

    fn parse_owned(parts: &http::request::Parts) -> crate::requests::http_request::HttpRequest {
        parse_owned_with_trust(parts, false)
    }

    fn parse_owned_with_trust(
        parts: &http::request::Parts,
        trust_forwarded_headers: bool,
    ) -> crate::requests::http_request::HttpRequest {
        crate::requests::http_request::HttpRequest::new(
            parts,
            uuid::Uuid::nil(),
            &crate::Ports {
                http: 80,
                https: 443,
            },
            false,
            trust_forwarded_headers,
        )
        .expect("parse owned request")
    }

    fn parse_view<'a>(
        parts: &'a http::request::Parts,
    ) -> crate::requests::http_request::HttpRequestView<'a> {
        parse_view_with_trust(parts, false)
    }

    fn parse_view_with_trust<'a>(
        parts: &'a http::request::Parts,
        trust_forwarded_headers: bool,
    ) -> crate::requests::http_request::HttpRequestView<'a> {
        crate::requests::http_request::HttpRequestView::new(
            parts,
            uuid::Uuid::nil(),
            &crate::Ports {
                http: 80,
                https: 443,
            },
            false,
            trust_forwarded_headers,
        )
        .expect("parse request view")
    }

    #[test]
    fn websocket_requires_connection_upgrade_in_parser() {
        let parts = build_request_parts("/ws", &[("Upgrade", "websocket")]);
        let owned = parse_owned(&parts);
        let view = parse_view(&parts);

        assert_eq!(owned.base_url.as_str(), "http://example.test");
        assert_eq!(view.base_url, "http://example.test");
        assert_eq!(owned.scheme.0.as_str(), "http");
        assert_eq!(view.scheme.0.as_str(), "http");
    }

    #[test]
    fn websocket_upgrade_maps_to_ws_scheme() {
        let parts = build_request_parts(
            "/ws",
            &[
                ("Upgrade", "websocket"),
                ("Connection", "keep-alive, Upgrade"),
            ],
        );
        let owned = parse_owned(&parts);
        let view = parse_view(&parts);

        assert_eq!(owned.base_url.as_str(), "ws://example.test");
        assert_eq!(view.base_url, "ws://example.test");
        assert_eq!(owned.scheme.0.as_str(), "http");
        assert_eq!(view.scheme.0.as_str(), "http");
    }

    #[test]
    fn websocket_upgrade_with_forwarded_https_maps_to_wss() {
        let parts = build_request_parts(
            "/ws",
            &[
                ("Upgrade", "websocket"),
                ("Connection", "Upgrade"),
                ("X-Forwarded-Proto", "https"),
            ],
        );
        let owned = parse_owned_with_trust(&parts, true);
        let view = parse_view_with_trust(&parts, true);

        assert_eq!(owned.base_url.as_str(), "wss://example.test");
        assert_eq!(view.base_url, "wss://example.test");
        assert_eq!(owned.scheme.0.as_str(), "https");
        assert_eq!(view.scheme.0.as_str(), "https");
    }

    #[test]
    fn direct_tls_request_maps_to_https_without_forwarded_headers() {
        let parts = build_request_parts("/index.yaml", &[]);
        let owned = crate::requests::http_request::HttpRequest::new(
            &parts,
            uuid::Uuid::nil(),
            &crate::Ports {
                http: 80,
                https: 443,
            },
            true,
            false,
        )
        .expect("parse owned direct tls request");
        let view = crate::requests::http_request::HttpRequestView::new(
            &parts,
            uuid::Uuid::nil(),
            &crate::Ports {
                http: 80,
                https: 443,
            },
            true,
            false,
        )
        .expect("parse direct tls request view");

        assert_eq!(owned.base_url.as_str(), "https://example.test");
        assert_eq!(view.base_url, "https://example.test");
        assert_eq!(owned.scheme.0.as_str(), "https");
        assert_eq!(view.scheme.0.as_str(), "https");
    }

    #[test]
    fn untrusted_forwarded_https_does_not_upgrade_scheme() {
        let parts = build_request_parts("/index.yaml", &[("X-Forwarded-Proto", "https")]);
        let owned = parse_owned(&parts);
        let view = parse_view(&parts);

        assert_eq!(owned.base_url.as_str(), "http://example.test");
        assert_eq!(view.base_url, "http://example.test");
        assert_eq!(owned.scheme.0.as_str(), "http");
        assert_eq!(view.scheme.0.as_str(), "http");
    }

    #[test]
    fn trusted_forwarded_https_upgrades_scheme() {
        let parts = build_request_parts("/index.yaml", &[("X-Forwarded-Proto", "https")]);
        let owned = crate::requests::http_request::HttpRequest::new(
            &parts,
            uuid::Uuid::nil(),
            &crate::Ports {
                http: 80,
                https: 443,
            },
            false,
            true,
        )
        .expect("parse owned trusted forwarded https request");
        let view = crate::requests::http_request::HttpRequestView::new(
            &parts,
            uuid::Uuid::nil(),
            &crate::Ports {
                http: 80,
                https: 443,
            },
            false,
            true,
        )
        .expect("parse trusted forwarded https request view");

        assert_eq!(owned.base_url.as_str(), "https://example.test");
        assert_eq!(view.base_url, "https://example.test");
        assert_eq!(owned.scheme.0.as_str(), "https");
        assert_eq!(view.scheme.0.as_str(), "https");
    }
}
