#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HttpQuery {
    pub path: crate::KsbhStr,
    pub params: Vec<(crate::KsbhStr, crate::KsbhStr)>,
}

#[derive(Debug, Clone)]
pub struct HttpQueryView<'a> {
    pub path: &'a str,
    pub params: Vec<(&'a str, &'a str)>,
}

impl<'a> HttpQueryView<'a> {
    pub fn new(
        req_header: &'a http::request::Parts,
    ) -> Result<Self, super::http_request::error::HttpRequestError> {
        let mut params: Vec<(&'a str, &'a str)> = Vec::with_capacity(12);

        if let Some(query_params) = req_header.uri.query() {
            for query_param in query_params.split('&') {
                let mut parts = query_param.splitn(2, '=');

                if let Some(k) = parts.next() {
                    let value = parts.next().unwrap_or("");

                    params.push((k, value));
                }
            }
        }

        Ok(Self {
            path: req_header.uri.path(),
            params,
        })
    }

    pub fn get_param(&self, param: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| *k == param)
            .map(|(_, v)| *v)
    }
}

impl HttpQuery {
    pub fn new(
        req_header: &http::request::Parts,
    ) -> Result<Self, super::http_request::error::HttpRequestError> {
        let mut params: Vec<(crate::KsbhStr, crate::KsbhStr)> = Vec::new();

        if let Some(query_params) = req_header.uri.query() {
            for query_with_param in query_params.split('&') {
                let mut parts = query_with_param.splitn(2, '=');

                if let Some(key) = parts.next() {
                    let value = parts.next().unwrap_or("");
                    params.push((crate::KsbhStr::new(key), crate::KsbhStr::new(value)));
                }
            }
        }

        Ok(Self {
            path: smol_str::SmolStr::new(req_header.uri.path()),
            params,
        })
    }

    pub fn to_owned(&self) -> Self {
        Self {
            path: self.path.clone(),
            params: self.params.clone(),
        }
    }

    pub fn get_param(&self, param: &str) -> Option<&crate::KsbhStr> {
        self.params
            .iter()
            .find(|(k, _)| k.as_str() == param)
            .map(|(_, v)| v)
    }
}

impl ::std::fmt::Display for HttpQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)?;

        if !self.params.is_empty() {
            write!(f, "?")?;

            for (i, (k, v)) in self.params.iter().enumerate() {
                if i > 0 {
                    write!(f, "&")?;
                }

                write!(f, "{}", k)?;
                if !v.is_empty() {
                    write!(f, "={}", v)?;
                }
            }
        }

        Ok(())
    }
}

impl<'a> ::std::fmt::Display for HttpQueryView<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)?;

        if !self.params.is_empty() {
            write!(f, "?")?;

            for (i, (k, v)) in self.params.iter().enumerate() {
                if i > 0 {
                    write!(f, "&")?;
                }

                write!(f, "{}", k)?;
                if !v.is_empty() {
                    write!(f, "={}", v)?;
                }
            }
        }

        Ok(())
    }
}
