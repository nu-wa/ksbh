#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfigIngress {
    pub name: String,
    pub host: String,
    pub tls: Option<FileConfigTLS>,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub excluded_modules: Vec<String>,
    pub https: Option<bool>,
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
