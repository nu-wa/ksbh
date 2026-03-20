#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub redis_url: Option<String>,
    #[serde(default)]
    pub cookie_key: Option<String>,
    #[serde(default)]
    pub constants: ConfigConstants,
    pub pyroscope_url: Option<String>,
    #[serde(default)]
    pub ports: ConfigPorts,
    #[serde(default)]
    pub listen_addresses: ConfigListenAddresses,
    #[serde(default)]
    pub config_paths: ConfigFilePaths,
    #[serde(default)]
    pub url_paths: ConfigURLPaths,
    #[serde(default = "default_threads")]
    pub threads: usize,
    #[serde(default)]
    pub performance: ConfigPerformance,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigConstants {
    #[serde(default)]
    pub tcp_fastopen_queue_size: usize,
    #[serde(default)]
    pub cookie_name: String,
    #[serde(default)]
    pub proxy_header_name: String,
    #[serde(default)]
    pub proxy_header_value: String,
}

impl Default for ConfigConstants {
    fn default() -> Self {
        Self {
            tcp_fastopen_queue_size: 12,
            cookie_name: "ksbh".to_string(),
            proxy_header_name: "Server".to_string(),
            proxy_header_value: "ksbh".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigPorts {
    #[serde(default = "default_ports_app")]
    pub app: ksbh_types::Ports,
    #[serde(default = "default_ports_external")]
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
    #[serde(default = "default_listen_http")]
    pub http: ::std::net::SocketAddr,
    #[serde(default = "default_listen_https")]
    pub https: ::std::net::SocketAddr,
    #[serde(default = "default_listen_internal")]
    pub internal: ::std::net::SocketAddr,
    #[serde(default = "default_listen_profiling")]
    pub profiling: ::std::net::SocketAddr,
    #[serde(default = "default_listen_prometheus")]
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

impl ConfigListenAddresses {
    pub fn internal_connect_addr(&self) -> ::std::net::SocketAddr {
        match self.internal {
            ::std::net::SocketAddr::V4(addr) => ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::LOCALHOST),
                addr.port(),
            ),
            ::std::net::SocketAddr::V6(addr) => ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V6(::std::net::Ipv6Addr::LOCALHOST),
                addr.port(),
            ),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigFilePaths {
    #[serde(default = "default_config_path_config")]
    pub config: ::std::path::PathBuf,
    #[serde(default = "default_config_path_modules")]
    pub modules: ::std::path::PathBuf,
    #[serde(default = "default_config_path_static_content")]
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
    #[serde(default = "default_url_path_modules")]
    pub modules: String,
}

impl Default for ConfigURLPaths {
    fn default() -> Self {
        Self {
            modules: "/_ksbh_internal/".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigPerformance {
    #[serde(default)]
    pub tcp_fastopen: Option<usize>,
    #[serde(default)]
    pub so_reuseport: Option<bool>,
    #[serde(default)]
    pub tcp_keepalive: Option<bool>,
}

impl Default for ConfigPerformance {
    fn default() -> Self {
        Self {
            tcp_fastopen: Some(12),
            so_reuseport: None,
            tcp_keepalive: None,
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
        let config_file_path = crate::utils::get_env_prefer_file("KSBH__CONFIG_PATHS__CONFIG")
            .unwrap_or("/app/ksbh/config.yaml".to_string());

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
        if let Some(url) = &self.redis_url
            && url.trim().is_empty()
        {
            return Err(ConfigError::ValidationError("redis_url cannot be empty"));
        }

        let cookie_key = self.cookie_key.as_ref().ok_or_else(|| {
            ConfigError::MissingMandatoryValue(
                "cookie_key must be provided via config or KSBH__COOKIE_KEY".to_string(),
            )
        })?;

        if cookie_key.trim().is_empty() {
            return Err(ConfigError::ValidationError("cookie_key cannot be empty"));
        }

        if crate::cookie::Key::try_from(cookie_key.as_bytes()).is_err() {
            return Err(ConfigError::ValidationError(
                "cookie_key must be at least 64 bytes",
            ));
        }

        if self.constants.cookie_name.trim().is_empty() {
            return Err(ConfigError::ValidationError(
                "constants.cookie_name cannot be empty",
            ));
        }

        http::header::HeaderName::from_bytes(self.constants.proxy_header_name.as_bytes())
            .map_err(|_| ConfigError::ValidationError("constants.proxy_header_name is invalid"))?;

        http::HeaderValue::from_str(&self.constants.proxy_header_value)
            .map_err(|_| ConfigError::ValidationError("constants.proxy_header_value is invalid"))?;

        Ok(())
    }

    pub fn to_server_conf(&self) -> pingora_core::server::configuration::ServerConf {
        pingora_core::server::configuration::ServerConf {
            daemon: false,
            ..Default::default()
        }
    }
}

fn default_threads() -> usize {
    8
}

fn default_ports_app() -> ksbh_types::Ports {
    ConfigPorts::default().app
}

fn default_ports_external() -> ksbh_types::Ports {
    ConfigPorts::default().external
}

fn default_listen_http() -> ::std::net::SocketAddr {
    ConfigListenAddresses::default().http
}

fn default_listen_https() -> ::std::net::SocketAddr {
    ConfigListenAddresses::default().https
}

fn default_listen_internal() -> ::std::net::SocketAddr {
    ConfigListenAddresses::default().internal
}

fn default_listen_profiling() -> ::std::net::SocketAddr {
    ConfigListenAddresses::default().profiling
}

fn default_listen_prometheus() -> ::std::net::SocketAddr {
    ConfigListenAddresses::default().prometheus
}

fn default_config_path_config() -> ::std::path::PathBuf {
    ConfigFilePaths::default().config
}

fn default_config_path_modules() -> ::std::path::PathBuf {
    ConfigFilePaths::default().modules
}

fn default_config_path_static_content() -> ::std::path::PathBuf {
    ConfigFilePaths::default().static_content
}

fn default_url_path_modules() -> String {
    ConfigURLPaths::default().modules
}
