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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_http_method_new() {
        let method = HttpMethod(http::Method::GET);
        assert_eq!(method.0, http::Method::GET);
    }

    #[test]
    fn test_http_method_display() {
        let method = HttpMethod(http::Method::GET);
        assert_eq!(format!("{}", method), "GET");
    }

    #[test]
    fn test_http_method_clone() {
        let method1 = HttpMethod(http::Method::POST);
        let method2 = method1.clone();
        assert_eq!(method1.0, method2.0);
    }

    #[test]
    fn test_http_method_equality() {
        let method1 = HttpMethod(http::Method::GET);
        let method2 = HttpMethod(http::Method::GET);
        let method3 = HttpMethod(http::Method::POST);
        assert_eq!(method1, method2);
        assert_ne!(method1, method3);
    }

    #[test]
    fn test_http_method_debug() {
        let method = HttpMethod(http::Method::DELETE);
        let debug_str = format!("{:?}", method);
        assert!(debug_str.contains("DELETE"));
    }

    #[test]
    fn test_http_method_serialize() {
        let method = HttpMethod(http::Method::GET);
        let serialized = serde_json::to_string(&method).unwrap();
        assert_eq!(serialized, "\"GET\"");
    }

    #[test]
    fn test_http_method_deserialize() {
        let json = "\"POST\"";
        let method: HttpMethod = serde_json::from_str(json).unwrap();
        assert_eq!(method.0, http::Method::POST);
    }

    #[test]
    fn test_http_method_view_new() {
        let view = HttpMethodView("GET");
        assert_eq!(view.0, "GET");
    }

    #[test]
    fn test_http_method_all_standard_methods() {
        let methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
        for method_str in methods {
            let method = HttpMethod(http::Method::from_bytes(method_str.as_bytes()).unwrap());
            assert_eq!(format!("{}", method), method_str);
        }
    }

    proptest! {
        #[test]
        fn test_http_method_serialize_deserialize_roundtrip(ref s in "[A-Z]+") {
            let method = HttpMethod(http::Method::from_bytes(s.as_bytes()).unwrap());
            let serialized = serde_json::to_string(&method).unwrap();
            let deserialized: HttpMethod = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(method, deserialized);
        }
    }
}
