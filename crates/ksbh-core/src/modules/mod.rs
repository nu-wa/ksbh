pub mod abi;
pub mod registry;

pub type ModuleConfigurationValues =
    ::std::sync::Arc<hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr>>;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Default,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(with = "String")]
pub enum ModuleConfigurationType {
    #[default]
    #[serde(alias = "RateLimit")]
    RateLimit,
    #[serde(alias = "HttpToHttps")]
    HttpToHttps,
    #[serde(alias = "RobotsDotTXT")]
    RobotsDotTXT,
    #[serde(alias = "OIDC")]
    OIDC,
    #[serde(alias = "POW")]
    POW,
    Custom(String),
}

impl ModuleConfigurationType {
    pub fn get_weight(&self) -> usize {
        match self {
            Self::OIDC => usize::MIN,
            Self::RateLimit => 1,
            Self::POW => 2,
            Self::Custom(_) => 3,
            Self::RobotsDotTXT => usize::MAX - 1,
            Self::HttpToHttps => usize::MAX,
        }
    }
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    kube::CustomResource,
    schemars::JsonSchema,
    Clone,
    Debug,
    PartialEq,
)]
#[kube(
    group = "modules.ksbh.rs",
    version = "v1",
    kind = "ModuleConfiguration",
    shortname = "mc",
    namespaced = false
)]
#[serde(rename_all = "camelCase")]
pub struct ModuleConfigurationSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: ModuleConfigurationType,
    #[schemars(default)]
    pub global: bool,
    #[serde(default = "default_true")]
    #[schemars(default = "default_true")]
    pub requires_proper_request: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<k8s_openapi::api::core::v1::SecretReference>,
    #[serde(default)]
    #[schemars(default)]
    pub requires_body: bool,
}

fn default_true() -> bool {
    true
}
