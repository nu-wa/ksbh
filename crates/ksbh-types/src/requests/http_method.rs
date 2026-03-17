#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HttpMethod(pub http::Method);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HttpMethodView<'a>(pub &'a str);

impl ::std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for HttpMethod {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for HttpMethod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let method = http::Method::from_bytes(s.as_bytes()).map_err(serde::de::Error::custom)?;
        Ok(HttpMethod(method))
    }
}
