#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigModules {
    #[serde(default)]
    pub global: bool,
    pub name: String,
    pub weight: i32,
    pub r#type: ksbh_core::modules::ModuleConfigurationType,
    #[serde(default)]
    pub requires_body: bool,
    #[serde(default)]
    pub config: ::std::collections::HashMap<String, String>,
}
