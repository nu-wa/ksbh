#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub pyroscope_url: Option<String>,
    pub ports: ConfigPorts,
    pub listen_addresses: ConfigListenAddresses,
    pub config_paths: ConfigFilePaths,
    pub url_paths: ConfigURLPaths,
    pub threads: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigPorts {
    pub app: ksbh_types::Ports,
    pub external: ksbh_types::Ports,
}

impl Default for ConfigPorts {
    fn default() -> Self {
        Self {
            app: ksbh_types::Ports {
                http: 8080,
                https: 8081,
            },
            external: ksbh_types::Ports {
                http: 80,
                https: 443,
            },
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigListenAddresses {
    pub http: ::std::net::SocketAddr,
    pub https: ::std::net::SocketAddr,
    pub internal: ::std::net::SocketAddr,
    pub profiling: ::std::net::SocketAddr,
    pub prometheus: ::std::net::SocketAddr,
}

impl Default for ConfigListenAddresses {
    fn default() -> Self {
        Self {
            http: ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(0, 0, 0, 0)),
                8080,
            ),
            https: ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(0, 0, 0, 0)),
                8081,
            ),
            internal: ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(0, 0, 0, 0)),
                8082,
            ),
            profiling: ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(0, 0, 0, 0)),
                8083,
            ),
            prometheus: ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(0, 0, 0, 0)),
                8084,
            ),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigFilePaths {
    pub config: ::std::path::PathBuf,
    pub modules: ::std::path::PathBuf,
    pub static_content: ::std::path::PathBuf,
}

impl Default for ConfigFilePaths {
    fn default() -> Self {
        Self {
            config: "/app/ksbh/config".into(),
            static_content: "/app/ksbh/config".into(),
            modules: "/app/ksbh/modules".into(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigURLPaths {
    pub modules: String,
}

impl Default for ConfigURLPaths {
    fn default() -> Self {
        Self {
            modules: "/_ksbh_internal/".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    ValidationError(&'static str),
    MissingMandatoryValue(String),
    ConfError(config::ConfigError),
    ParsingError(String),
}

impl ::std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConfigError: {}",
            match self {
                ConfigError::ConfError(e) => e.to_string(),
                ConfigError::MissingMandatoryValue(e) => e.to_string(),
                ConfigError::ParsingError(e) => e.to_string(),
                ConfigError::ValidationError(e) => e.to_string(),
            }
        )
    }
}

impl ::std::error::Error for ConfigError {}

impl From<config::ConfigError> for ConfigError {
    fn from(value: config::ConfigError) -> Self {
        Self::ConfError(value)
    }
}

impl From<Box<dyn ::std::error::Error + 'static>> for ConfigError {
    fn from(value: Box<dyn ::std::error::Error + 'static>) -> Self {
        Self::ParsingError(value.to_string())
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_file_path = crate::utils::get_env_prefer_file("KSBH_CONFIG_FILE")
            .unwrap_or("/app/ksbh-config.yaml".to_string());

        let cfg = config::Config::builder()
            .add_source(config::File::with_name(&config_file_path).required(false))
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .prefix("KSBH"),
            )
            .build()?;

        let cfg: Self = cfg.try_deserialize()?;

        cfg.validate()?;

        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        // TODO: implement checks for file paths, if they're valid (openable/readable).
        if self.redis_url.trim().is_empty() {
            return Err(ConfigError::ValidationError("redis_url cannot be empty"));
        }

        Ok(())
    }

    pub fn to_server_conf(&self) -> pingora::server::configuration::ServerConf {
        pingora::server::configuration::ServerConf {
            daemon: false,
            ..Default::default()
        }
    }
}
