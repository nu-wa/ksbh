#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HttpScheme(pub http::uri::Scheme);

impl serde::Serialize for HttpScheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for HttpScheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::str::FromStr;
        let s = String::deserialize(deserializer)?;
        let method = http::uri::Scheme::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(HttpScheme(method))
    }
}

impl ::std::fmt::Display for HttpScheme {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}
