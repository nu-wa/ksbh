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
        serializer.serialize_str(&self.to_storage_key())
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
            module_name: smol_str::SmolStr::new(module_name),
            session_id,
        })
    }
}

impl ModuleSessionKey {
    pub fn to_storage_key(&self) -> String {
        format!("{}:{}", self.module_name, self.session_id)
    }
}
