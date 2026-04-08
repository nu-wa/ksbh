#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigIngress {
    pub name: String,
    pub host: String,
    pub tls: Option<FileConfigTLS>,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub excluded_modules: Vec<String>,
    #[serde(default)]
    pub peer_options: Option<FileConfigPeerOptions>,
    pub paths: Vec<FileConfigPaths>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigTLS {
    pub cert_file: String,
    pub key_file: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigPaths {
    pub path: String,
    pub r#type: String,
    pub backend: String,
    pub service: Option<FileConfigService>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigService {
    pub name: String,
    pub port: u16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigPeerOptions {
    #[serde(default)]
    pub sni: Option<String>,
    #[serde(default)]
    pub alternative_names: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub verify_cert: bool,
}

fn default_true() -> bool {
    true
}
