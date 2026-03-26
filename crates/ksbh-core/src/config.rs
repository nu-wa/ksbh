/// Root configuration for the KSBH proxy server.
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
    #[serde(default, deserialize_with = "deserialize_trusted_proxies")]
    pub trusted_proxies: Vec<ipnet::IpNet>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigConstants {
    #[serde(default = "default_tcp_fastopen_queue_size")]
    pub tcp_fastopen_queue_size: usize,
    #[serde(default = "default_cookie_name")]
    pub cookie_name: String,
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
    #[serde(default = "default_proxy_header_name")]
    pub proxy_header_name: String,
    #[serde(default = "default_proxy_header_value")]
    pub proxy_header_value: String,
}

impl Default for ConfigConstants {
    fn default() -> Self {
        Self {
            tcp_fastopen_queue_size: 12,
            cookie_name: "ksbh".to_string(),
            cookie_secure: true,
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
            config: "/app/config/config.yaml".into(),
            static_content: "/app/data/static".into(),
            modules: "/app/modules".into(),
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

/// Errors that can occur during configuration loading and validation.
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
    /// Loads configuration from YAML file and environment variables.
    ///
    /// Precedence (highest to lowest): environment variables (prefix `KSBH__`),
    /// YAML file at path specified by `KSBH__CONFIG_PATHS__CONFIG` or default
    /// `/app/config/config.yaml`.
    pub fn load() -> Result<Self, ConfigError> {
        let config_file_path = crate::utils::get_env_prefer_file("KSBH__CONFIG_PATHS__CONFIG")
            .unwrap_or("/app/config/config.yaml".to_string());

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

    pub fn trusts_forwarded_headers_from(&self, client_ip: Option<::std::net::IpAddr>) -> bool {
        let Some(client_ip) = client_ip else {
            return false;
        };

        self.trusted_proxies
            .iter()
            .any(|network| network.contains(&client_ip))
    }

    /// Converts to a Pingora server configuration.
    ///
    /// Used to initialize the Pingora server with KSBH-specific settings.
    pub fn to_server_conf(&self) -> pingora_core::server::configuration::ServerConf {
        pingora_core::server::configuration::ServerConf {
            daemon: false,
            ..Default::default()
        }
    }
}

fn deserialize_trusted_proxies<'de, D>(deserializer: D) -> Result<Vec<ipnet::IpNet>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum TrustedProxiesInput {
        Sequence(Vec<String>),
        IndexedMap(::std::collections::BTreeMap<String, String>),
    }

    let values = match <TrustedProxiesInput as serde::Deserialize>::deserialize(deserializer)? {
        TrustedProxiesInput::Sequence(values) => values,
        TrustedProxiesInput::IndexedMap(values) => values.into_values().collect(),
    };

    values
        .into_iter()
        .map(|value| {
            value
                .parse::<ipnet::IpNet>()
                .or_else(|_| value.parse::<::std::net::IpAddr>().map(ipnet::IpNet::from))
                .map_err(|_| {
                    serde::de::Error::custom(format!(
                        "invalid trusted proxy '{value}', expected IP or CIDR"
                    ))
                })
        })
        .collect()
}

fn default_threads() -> usize {
    8
}

fn default_tcp_fastopen_queue_size() -> usize {
    12
}

fn default_cookie_name() -> String {
    "ksbh".to_string()
}

fn default_cookie_secure() -> bool {
    true
}

fn default_proxy_header_name() -> String {
    "Server".to_string()
}

fn default_proxy_header_value() -> String {
    "ksbh".to_string()
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

#[cfg(test)]
mod tests {
    #[test]
    fn trusted_proxies_accept_ip_and_cidr_strings() {
        let cfg: crate::Config = serde_yaml_bw::from_str(
            r#"
cookie_key: "0123456789012345678901234567890101234567890123456789012345678901"
pyroscope_url: null
trusted_proxies:
  - "10.0.0.10"
  - "192.168.0.0/24"
"#,
        )
        .expect("deserialize config with trusted proxies");

        assert_eq!(cfg.trusted_proxies.len(), 2);
        let trusted_ip: ::std::net::IpAddr = "10.0.0.10".parse().expect("parse trusted proxy ip");
        assert!(cfg.trusted_proxies[0].contains(&trusted_ip));
        let contained_ip: ::std::net::IpAddr =
            "192.168.0.42".parse().expect("parse cidr contained ip");
        assert!(cfg.trusted_proxies[1].contains(&contained_ip));
    }

    #[test]
    fn trusted_proxies_accept_env_indexed_map_shape() {
        let cfg: crate::Config = serde_json::from_value(serde_json::json!({
            "cookie_key": "0123456789012345678901234567890101234567890123456789012345678901",
            "pyroscope_url": null,
            "trusted_proxies": {
                "0": "10.0.0.10",
                "1": "192.168.0.0/24"
            }
        }))
        .expect("deserialize config with env-style trusted proxies");

        assert_eq!(cfg.trusted_proxies.len(), 2);
        let trusted_ip: ::std::net::IpAddr = "10.0.0.10".parse().expect("parse trusted proxy ip");
        assert!(cfg.trusted_proxies[0].contains(&trusted_ip));
        let contained_ip: ::std::net::IpAddr =
            "192.168.0.42".parse().expect("parse cidr contained ip");
        assert!(cfg.trusted_proxies[1].contains(&contained_ip));
    }

    #[test]
    fn trusted_forwarded_headers_require_proxy_match() {
        let cfg = crate::Config {
            redis_url: None,
            cookie_key: Some(
                "0123456789012345678901234567890101234567890123456789012345678901".to_string(),
            ),
            constants: super::ConfigConstants::default(),
            pyroscope_url: None,
            ports: super::ConfigPorts::default(),
            listen_addresses: super::ConfigListenAddresses::default(),
            config_paths: super::ConfigFilePaths::default(),
            url_paths: super::ConfigURLPaths::default(),
            threads: 8,
            performance: super::ConfigPerformance::default(),
            trusted_proxies: vec!["10.0.0.0/8".parse().expect("parse trusted proxy network")],
        };

        assert!(cfg.trusts_forwarded_headers_from(Some(
            "10.1.2.3".parse().expect("parse trusted client address"),
        )));
        assert!(
            !cfg.trusts_forwarded_headers_from(Some(
                "192.168.1.1"
                    .parse()
                    .expect("parse untrusted client address"),
            ))
        );
        assert!(!cfg.trusts_forwarded_headers_from(None));
    }
}
