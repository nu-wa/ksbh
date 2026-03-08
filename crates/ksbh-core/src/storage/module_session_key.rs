pub const MODULE_SESSION_RESERVED: &str = "_ksbh_session";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleSessionKey {
    module_name: String,
    session_id: uuid::Uuid,
}

impl ModuleSessionKey {
    pub fn new(module_name: &str, session_id: uuid::Uuid) -> Self {
        Self {
            module_name: module_name.to_string(),
            session_id,
        }
    }

    pub fn user_session(session_id: uuid::Uuid) -> Self {
        Self {
            module_name: MODULE_SESSION_RESERVED.to_string(),
            session_id,
        }
    }

    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    pub fn session_id(&self) -> uuid::Uuid {
        self.session_id
    }
}

impl serde::Serialize for ModuleSessionKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}:{}", self.module_name, self.session_id))
    }
}

impl<'de> serde::Deserialize<'de> for ModuleSessionKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let (module_name, session_id_str) = s
            .split_once(':')
            .ok_or_else(|| serde::de::Error::custom("invalid ModuleSessionKey format"))?;
        let session_id = uuid::Uuid::parse_str(session_id_str)
            .map_err(|e| serde::de::Error::custom(format!("invalid UUID: {}", e)))?;
        Ok(Self {
            module_name: module_name.to_string(),
            session_id,
        })
    }
}

impl ModuleSessionKey {
    pub fn as_str(&self) -> String {
        format!("{}:{}", self.module_name, self.session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_session_key_new() {
        let session_id = uuid::Uuid::new_v4();
        let key = ModuleSessionKey::new("oidc", session_id);
        assert_eq!(key.module_name(), "oidc");
        assert_eq!(key.session_id(), session_id);
    }

    #[test]
    fn test_module_session_key_user_session() {
        let session_id = uuid::Uuid::new_v4();
        let key = ModuleSessionKey::user_session(session_id);
        assert_eq!(key.module_name(), MODULE_SESSION_RESERVED);
        assert_eq!(key.session_id(), session_id);
    }

    #[test]
    fn test_module_session_key_as_str() {
        let session_id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = ModuleSessionKey::new("rate_limit", session_id);
        assert_eq!(
            key.as_str(),
            "rate_limit:550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_module_session_key_clone() {
        let session_id = uuid::Uuid::new_v4();
        let key1 = ModuleSessionKey::new("test", session_id);
        let key2 = key1.clone();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_module_session_key_debug() {
        let session_id = uuid::Uuid::new_v4();
        let key = ModuleSessionKey::new("test", session_id);
        let debug_str = format!("{:?}", key);
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_module_session_key_equality() {
        let session_id = uuid::Uuid::new_v4();
        let key1 = ModuleSessionKey::new("test", session_id);
        let key2 = ModuleSessionKey::new("test", session_id);
        let key3 = ModuleSessionKey::new("other", session_id);
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_module_session_key_hash() {
        use std::collections::HashSet;
        let session_id = uuid::Uuid::new_v4();
        let mut set = HashSet::new();
        set.insert(ModuleSessionKey::new("test1", session_id));
        set.insert(ModuleSessionKey::new("test2", session_id));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_module_session_key_serialize() {
        let session_id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = ModuleSessionKey::new("oidc", session_id);
        let serialized = serde_json::to_string(&key).unwrap();
        assert_eq!(serialized, "\"oidc:550e8400-e29b-41d4-a716-446655440000\"");
    }

    #[test]
    fn test_module_session_key_deserialize() {
        let json = "\"rate_limit:550e8400-e29b-41d4-a716-446655440000\"";
        let key: ModuleSessionKey = serde_json::from_str(json).unwrap();
        assert_eq!(key.module_name(), "rate_limit");
    }

    #[test]
    fn test_module_session_key_deserialize_invalid_format() {
        let json = "\"invalid\"";
        let result: Result<ModuleSessionKey, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_module_session_key_deserialize_invalid_uuid() {
        let json = "\"test:not-a-uuid\"";
        let result: Result<ModuleSessionKey, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_module_session_key_serialize_deserialize_roundtrip() {
        let session_id = uuid::Uuid::new_v4();
        let key = ModuleSessionKey::new("test_module", session_id);
        let serialized = serde_json::to_string(&key).unwrap();
        let deserialized: ModuleSessionKey = serde_json::from_str(&serialized).unwrap();
        assert_eq!(key, deserialized);
    }
}
