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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::str::FromStr;

    #[test]
    fn test_http_scheme_http() {
        let scheme = HttpScheme(http::uri::Scheme::HTTP);
        assert_eq!(scheme.0, http::uri::Scheme::HTTP);
    }

    #[test]
    fn test_http_scheme_https() {
        let scheme = HttpScheme(http::uri::Scheme::HTTPS);
        assert_eq!(scheme.0, http::uri::Scheme::HTTPS);
    }

    #[test]
    fn test_http_scheme_clone() {
        let scheme1 = HttpScheme(http::uri::Scheme::HTTP);
        let scheme2 = scheme1.clone();
        assert_eq!(scheme1.0, scheme2.0);
    }

    #[test]
    fn test_http_scheme_equality() {
        let scheme1 = HttpScheme(http::uri::Scheme::HTTP);
        let scheme2 = HttpScheme(http::uri::Scheme::HTTP);
        let scheme3 = HttpScheme(http::uri::Scheme::HTTPS);
        assert_eq!(scheme1, scheme2);
        assert_ne!(scheme1, scheme3);
    }

    #[test]
    fn test_http_scheme_debug() {
        let scheme = HttpScheme(http::uri::Scheme::HTTP);
        let debug_str = format!("{:?}", scheme);
        assert!(debug_str.contains("http"));
    }

    #[test]
    fn test_http_scheme_serialize_http() {
        let scheme = HttpScheme(http::uri::Scheme::HTTP);
        let serialized = serde_json::to_string(&scheme).unwrap();
        assert_eq!(serialized, "\"http\"");
    }

    #[test]
    fn test_http_scheme_serialize_https() {
        let scheme = HttpScheme(http::uri::Scheme::HTTPS);
        let serialized = serde_json::to_string(&scheme).unwrap();
        assert_eq!(serialized, "\"https\"");
    }

    #[test]
    fn test_http_scheme_deserialize_http() {
        let json = "\"http\"";
        let scheme: HttpScheme = serde_json::from_str(json).unwrap();
        assert_eq!(scheme.0, http::uri::Scheme::HTTP);
    }

    #[test]
    fn test_http_scheme_deserialize_https() {
        let json = "\"https\"";
        let scheme: HttpScheme = serde_json::from_str(json).unwrap();
        assert_eq!(scheme.0, http::uri::Scheme::HTTPS);
    }

    #[test]
    fn test_http_scheme_from_str_valid() {
        use std::str::FromStr;
        let scheme = http::uri::Scheme::from_str("http").unwrap();
        assert_eq!(scheme.as_str(), "http");
    }

    #[test]
    fn test_http_scheme_from_str_https() {
        use std::str::FromStr;
        let scheme = http::uri::Scheme::from_str("https").unwrap();
        assert_eq!(scheme.as_str(), "https");
    }

    proptest! {
        #[test]
        fn test_http_scheme_serialize_deserialize(ref s in "https?") {
            let scheme_internal = http::uri::Scheme::from_str(s).unwrap();
            let scheme = HttpScheme(scheme_internal);
            let serialized = serde_json::to_string(&scheme).unwrap();
            let deserialized: HttpScheme = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(scheme.0.as_str(), deserialized.0.as_str());
        }
    }
}
