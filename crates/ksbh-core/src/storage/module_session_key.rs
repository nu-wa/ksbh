pub const MODULE_SESSION_RESERVED: &str = "_ksbh_internal";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleSessionKey {
    module_name: smol_str::SmolStr,
    session_id: uuid::Uuid,
}

impl ModuleSessionKey {
    pub fn new(module_name: &str, session_id: uuid::Uuid) -> Self {
        Self {
            module_name: smol_str::SmolStr::new(module_name),
            session_id,
        }
    }

    pub fn user_session(session_id: uuid::Uuid) -> Self {
        Self {
            module_name: smol_str::SmolStr::new(MODULE_SESSION_RESERVED),
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
        use serde::ser::SerializeTuple;
        let mut t = serializer.serialize_tuple(2)?;
        t.serialize_element(self.module_name.as_str())?;
        t.serialize_element(&self.session_id.to_string())?;
        t.end()
    }
}

impl<'de> serde::Deserialize<'de> for ModuleSessionKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ModuleSessionKey;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a tuple of (module_name, session_id)")
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let module_name: String = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::custom("missing module_name"))?;
                let session_id_str: String = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::custom("missing session_id"))?;
                let session_id = uuid::Uuid::parse_str(&session_id_str)
                    .map_err(|e| serde::de::Error::custom(format!("invalid UUID: {}", e)))?;
                Ok(ModuleSessionKey {
                    module_name: smol_str::SmolStr::new(&module_name),
                    session_id,
                })
            }
        }
        deserializer.deserialize_tuple(2, Visitor)
    }
}

impl ModuleSessionKey {
    pub fn to_storage_key(&self) -> String {
        format!("{}:{}", self.module_name, self.session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let key = ModuleSessionKey::new("oidc:flow_state", uuid::Uuid::new_v4());
        let encoded = rmp_serde::to_vec(&key).unwrap();
        let decoded: ModuleSessionKey = rmp_serde::from_slice(&encoded).unwrap();
        assert_eq!(decoded.module_name(), "oidc:flow_state");
        assert_eq!(decoded.session_id(), key.session_id());
    }

    #[test]
    fn serialize_deserialize_shared_key() {
        let key = ModuleSessionKey::new("__ksbh_shared__:user_id", uuid::Uuid::new_v4());
        let encoded = rmp_serde::to_vec(&key).unwrap();
        let decoded: ModuleSessionKey = rmp_serde::from_slice(&encoded).unwrap();
        assert_eq!(decoded.module_name(), "__ksbh_shared__:user_id");
        assert_eq!(decoded.session_id(), key.session_id());
    }
}
