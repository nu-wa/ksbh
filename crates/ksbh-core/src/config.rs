//! ksbh configuration
//!
//! ksbh configuration is, for now, built from environment variables.
//!
//! Required environment variables:
//! * `DATABASE_URL`
//! * `REDIS_URL`
//! * `ksbh_MOD_JWT_SECRET`
//!
//! Other environment variables with default values:
//! * `KSBH_CONFIG_DIRECTORY="/app/.config"`
//! * `KSBH_CACHE_DIRECTORY="/app/.cache"`
//! * `KSBH_PLUGINS_DIR="/app/plugins"`
//! * `KSBH_SERVE_DIRECTORY="/app/html"`
//! * `KSBH_EXTISM_CACHE_DIRECTORY="${ksbh_CACHE_DIRECTORY}/wasmtime/"`
//! * `KSBH_EXTISM_CACHE_CLEANUP_INTERVAL="30m"`
//! * `KSBH_EXTISM_CACHE_TOTAL_SIZE_LIMIT="1Gi"`
//! * `EXTISM_CACHE_CONFIG="${KSBH_CONFIG_DIRECTORY}/wasmtime/config.toml"`
//! * `DEBUG_LEVEL="INFO"`

/// ksbh configuration, for now we're building from Environment variables, with default
/// values if the variable is missing.
#[derive(Debug, Clone)]
pub struct Config {
    pub http_port: u16,
    pub https_port: u16,
    pub ext_http_port: u16,
    pub ext_https_port: u16,
    pub listen_address: ::std::net::SocketAddr,
    pub listen_address_tls: ::std::net::SocketAddr,
    pub listen_address_api: ::std::net::SocketAddr,
    pub listen_address_prom: ::std::net::SocketAddr,
    pub listen_address_internal: ::std::net::SocketAddr,
    pub listen_address_profiling: ::std::net::SocketAddr,
    pub plugins_directory: ::std::path::PathBuf,
    pub database_url: String,
    pub redis_url: String,
    pub threads: usize,
    pub config_directory: ::std::path::PathBuf,
    pub serve_directory: ::std::path::PathBuf,
    pub web_app_mode: ConfigWebAppMode,
    pub modules_internal_path: smol_str::SmolStr,
    pub modules_directory: ::std::path::PathBuf,
    pub pyroscope_url: Option<ksbh_types::KsbhStr>,
}

#[derive(Debug, Clone)]
pub enum ConfigWebAppMode {
    SubDomain(smol_str::SmolStr),
    SubPath(smol_str::SmolStr),
}

impl ConfigWebAppMode {
    pub fn load() -> Self {
        match crate::utils::get_env_prefer_file("WEB_APP_SERVE_MODE") {
            Err(_) => {
                Self::SubDomain(smol_str::SmolStr::from(crate::utils::get_env_prefer_file("WEB_APP_SERVE_SUBDOMAIN").expect("When setting WEB_APP_SERVE_MODE to subdomain (or nothing as it defaults to subdomain) needs WEB_APP_SERVE_SUBDOMAIN to be set!")))
            },
            Ok(mode) => {
                if mode.to_lowercase() == "subdomain" {
                    Self::SubDomain(smol_str::SmolStr::new(crate::utils::get_env_prefer_file("WEB_APP_SERVE_SUBDOMAIN").expect("When setting WEB_APP_SERVE_MODE to subdomain (or nothing as it defaults to subdomain) needs WEB_APP_SERVE_SUBDOMAIN to be set!")))
                } else if mode.to_lowercase() == "subpath" {
                    Self::SubDomain(smol_str::SmolStr::new(crate::utils::get_env_prefer_file("WEB_APP_SERVE_SUBPATH").expect("When setting WEB_APP_SERVE_MODE to subpath needs WEB_APP_SERVE_SUBPATH to be set!")))
                } else {
                   panic!("Invalid WEB_APP_SERVE_MODE mode supported modes are Subdomain|Subpath"); 
                }
            }
        }
    }
}

impl Config {
    // TODO: Make this return a Result instead, and impl Default with this ?
    pub fn build_from_env() -> Self {
        let http_port = Self::get_port("HTTP_PORT", 8080);
        let https_port = Self::get_port("HTTPS_PORT", 8081);
        let api_port = Self::get_port("API_PORT", 8082);
        let prom_port = Self::get_port("PROM_PORT", 8089);
        let profiling_port = Self::get_port("PROFILING_PORT", 8090);
        let internal_port = Self::get_port("INTERNAL_PORT", 9000);

        Self {
            http_port,
            https_port,
            ext_http_port: Self::get_port("EXT_HTTP_PORT", 80),
            ext_https_port: Self::get_port("EXT_HTTPS_PORT", 443),
            database_url: crate::utils::get_env_prefer_file("DATABASE_URL")
                .expect("DATABASE_URL env variable missing"),
            redis_url: crate::utils::get_env_prefer_file("REDIS_URL")
                .expect("REDIS_URL env variable missing"),
            plugins_directory: ::std::path::PathBuf::from(
                crate::utils::get_env_prefer_file("KSBH_PLUGINS_DIR")
                    .unwrap_or("/app/data/plugins".to_string()),
            ),
            listen_address: Self::get_listen_address("HTTP_LISTEN_ADDRESS", http_port),
            listen_address_tls: Self::get_listen_address("HTTPS_LISTEN_ADDRESS", https_port),
            listen_address_api: Self::get_listen_address("LISTEN_ADDRESS_STATIC", api_port),
            listen_address_prom: Self::get_listen_address("PROM_LISTEN_ADDRESS", prom_port),
            listen_address_profiling: Self::get_listen_address(
                "PROFILING_LISTEN_ADDRESS",
                profiling_port,
            ),
            listen_address_internal: Self::get_listen_address(
                "LISTEN_ADDRESS_INTERNAL",
                internal_port,
            ),
            threads: crate::utils::get_env_prefer_file("KSBH_SERVICES_THREADS").map_or(
                8,
                |threads| {
                    threads
                        .parse::<usize>()
                        .expect("KSBH_SERVICES_THREADS is invalid")
                },
            ),
            modules_directory: crate::utils::get_env_prefer_file("KSBH_MODULES_DIRECTORY").map_or(
                ::std::path::PathBuf::from("/app/data/modules"),
                ::std::path::PathBuf::from,
            ),
            config_directory: crate::utils::get_env_prefer_file("KSBH_CONFIG_DIRECTORY").map_or(
                ::std::path::PathBuf::from("/app/.config"),
                ::std::path::PathBuf::from,
            ),
            serve_directory: crate::utils::get_env_prefer_file("KSBH_SERVE_DIRECTORY").map_or(
                ::std::path::PathBuf::from("/app/html"),
                ::std::path::PathBuf::from,
            ),
            web_app_mode: ConfigWebAppMode::load(),
            modules_internal_path: smol_str::SmolStr::new(
                crate::utils::get_env_prefer_file("KSBH_MODULES_INTERNAL_PATH")
                    .unwrap_or("/_ksbh_internal/".into()),
            ),
            pyroscope_url: crate::utils::get_env_prefer_file("PYROSCOPE_URL")
                .ok()
                .map(ksbh_types::KsbhStr::new),
        }
    }

    pub fn to_server_conf(&self) -> pingora::server::configuration::ServerConf {
        pingora::server::configuration::ServerConf {
            daemon: false,
            ..Default::default()
        }
    }

    pub fn to_public(&self) -> ksbh_types::PublicConfig {
        ksbh_types::PublicConfig {
            http_port: Self::get_port("EXT_HTTP_PORT", 80),
            https_port: Self::get_port("EXT_HTTPS_PORT", 443),
        }
    }

    fn get_listen_address(key: &str, port: u16) -> ::std::net::SocketAddr {
        let l_address = match crate::utils::get_env_prefer_file(key) {
            Ok(l_address) => match l_address.parse::<::std::net::SocketAddr>() {
                Ok(l_address) => Some(l_address),
                Err(e) => {
                    tracing::error!("{e}");
                    None
                }
            },
            Err(_) => {
                tracing::info!("{key} env variable not found defaulting to '0.0.0.0:{port}'");
                None
            }
        };

        match l_address {
            Some(l_address) => l_address,
            None => ::std::net::SocketAddr::new(
                ::std::net::IpAddr::V4(::std::net::Ipv4Addr::new(0, 0, 0, 0)),
                port,
            ),
        }
    }

    fn get_port(key: &str, default_value: u16) -> u16 {
        match crate::utils::get_env_prefer_file(key) {
            Ok(http_port) => match http_port.parse::<u16>() {
                Ok(http_port) => http_port,
                Err(_) => {
                    tracing::warn!(
                        "Could not parse {key} env variable, defaulting to {default_value}"
                    );
                    default_value
                }
            },
            Err(_) => {
                tracing::info!("{key} env variable not found defaulting to {default_value}");

                default_value
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smol_str::SmolStr;

    #[test]
    fn test_config_web_app_mode_subdomain() {
        let mode = ConfigWebAppMode::SubDomain(SmolStr::new("example.com"));
        match mode {
            ConfigWebAppMode::SubDomain(host) => {
                assert_eq!(host.as_str(), "example.com");
            }
            _ => panic!("Expected SubDomain"),
        }
    }

    #[test]
    fn test_config_web_app_mode_subpath() {
        let mode = ConfigWebAppMode::SubPath(SmolStr::new("/app"));
        match mode {
            ConfigWebAppMode::SubPath(path) => {
                assert_eq!(path.as_str(), "/app");
            }
            _ => panic!("Expected SubPath"),
        }
    }

    #[test]
    fn test_config_web_app_mode_debug() {
        let mode = ConfigWebAppMode::SubDomain(SmolStr::new("example.com"));
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("SubDomain"));
        assert!(debug_str.contains("example.com"));
    }

    #[test]
    fn test_config_web_app_mode_clone() {
        let mode1 = ConfigWebAppMode::SubDomain(SmolStr::new("example.com"));
        let mode2 = mode1.clone();
        match (mode1, mode2) {
            (ConfigWebAppMode::SubDomain(h1), ConfigWebAppMode::SubDomain(h2)) => {
                assert_eq!(h1.as_str(), h2.as_str());
            }
            _ => panic!("Expected both to be SubDomain"),
        }
    }

    #[test]
    fn test_config_to_server_conf() {
        let config = Config {
            http_port: 8080,
            https_port: 8443,
            ext_http_port: 80,
            ext_https_port: 443,
            listen_address: "0.0.0.0:8080".parse().unwrap(),
            listen_address_tls: "0.0.0.0:8443".parse().unwrap(),
            listen_address_api: "0.0.0.0:8081".parse().unwrap(),
            listen_address_prom: "0.0.0.0:9090".parse().unwrap(),
            listen_address_internal: "0.0.0.0:8082".parse().unwrap(),
            listen_address_profiling: "0.0.0.0:6060".parse().unwrap(),
            plugins_directory: ::std::path::PathBuf::from("/tmp/plugins"),
            database_url: "postgres://localhost/db".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            threads: 4,
            config_directory: ::std::path::PathBuf::from("/tmp/config"),
            serve_directory: ::std::path::PathBuf::from("/tmp/serve"),
            web_app_mode: ConfigWebAppMode::SubDomain(SmolStr::new("example.com")),
            modules_internal_path: SmolStr::new("/_ksbh_internal"),
            modules_directory: ::std::path::PathBuf::from("/tmp/modules"),
            pyroscope_url: None,
        };

        let server_conf = config.to_server_conf();
        assert!(!server_conf.daemon);
    }

    #[test]
    fn test_config_to_public() {
        let config = Config {
            http_port: 8080,
            https_port: 8443,
            ext_http_port: 80,
            ext_https_port: 443,
            listen_address: "0.0.0.0:8080".parse().unwrap(),
            listen_address_tls: "0.0.0.0:8443".parse().unwrap(),
            listen_address_api: "0.0.0.0:8081".parse().unwrap(),
            listen_address_prom: "0.0.0.0:9090".parse().unwrap(),
            listen_address_internal: "0.0.0.0:8082".parse().unwrap(),
            listen_address_profiling: "0.0.0.0:6060".parse().unwrap(),
            plugins_directory: ::std::path::PathBuf::from("/tmp/plugins"),
            database_url: "postgres://localhost/db".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            threads: 4,
            config_directory: ::std::path::PathBuf::from("/tmp/config"),
            serve_directory: ::std::path::PathBuf::from("/tmp/serve"),
            web_app_mode: ConfigWebAppMode::SubDomain(SmolStr::new("example.com")),
            modules_internal_path: SmolStr::new("/_ksbh_internal"),
            modules_directory: ::std::path::PathBuf::from("/tmp/modules"),
            pyroscope_url: None,
        };

        let public = config.to_public();
        assert_eq!(public.http_port, 80);
        assert_eq!(public.https_port, 443);
    }

    #[test]
    fn test_config_debug() {
        let config = Config {
            http_port: 8080,
            https_port: 8443,
            ext_http_port: 80,
            ext_https_port: 443,
            listen_address: "0.0.0.0:8080".parse().unwrap(),
            listen_address_tls: "0.0.0.0:8443".parse().unwrap(),
            listen_address_api: "0.0.0.0:8081".parse().unwrap(),
            listen_address_prom: "0.0.0.0:9090".parse().unwrap(),
            listen_address_internal: "0.0.0.0:8082".parse().unwrap(),
            listen_address_profiling: "0.0.0.0:6060".parse().unwrap(),
            plugins_directory: ::std::path::PathBuf::from("/tmp/plugins"),
            database_url: "postgres://localhost/db".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            threads: 4,
            config_directory: ::std::path::PathBuf::from("/tmp/config"),
            serve_directory: ::std::path::PathBuf::from("/tmp/serve"),
            web_app_mode: ConfigWebAppMode::SubDomain(SmolStr::new("example.com")),
            modules_internal_path: SmolStr::new("/_ksbh_internal"),
            modules_directory: ::std::path::PathBuf::from("/tmp/modules"),
            pyroscope_url: None,
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("http_port"));
        assert!(debug_str.contains("8080"));
    }

    #[test]
    fn test_config_clone() {
        let config1 = Config {
            http_port: 8080,
            https_port: 8443,
            ext_http_port: 80,
            ext_https_port: 443,
            listen_address: "0.0.0.0:8080".parse().unwrap(),
            listen_address_tls: "0.0.0.0:8443".parse().unwrap(),
            listen_address_api: "0.0.0.0:8081".parse().unwrap(),
            listen_address_prom: "0.0.0.0:9090".parse().unwrap(),
            listen_address_internal: "0.0.0.0:8082".parse().unwrap(),
            listen_address_profiling: "0.0.0.0:6060".parse().unwrap(),
            plugins_directory: ::std::path::PathBuf::from("/tmp/plugins"),
            database_url: "postgres://localhost/db".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            threads: 4,
            config_directory: ::std::path::PathBuf::from("/tmp/config"),
            serve_directory: ::std::path::PathBuf::from("/tmp/serve"),
            web_app_mode: ConfigWebAppMode::SubDomain(SmolStr::new("example.com")),
            modules_internal_path: SmolStr::new("/_ksbh_internal"),
            modules_directory: ::std::path::PathBuf::from("/tmp/modules"),
            pyroscope_url: None,
        };

        let config2 = config1.clone();
        assert_eq!(config1.http_port, config2.http_port);
        assert_eq!(config1.redis_url, config2.redis_url);
    }

    #[test]
    fn test_config_with_pyroscope_url() {
        let config = Config {
            http_port: 8080,
            https_port: 8443,
            ext_http_port: 80,
            ext_https_port: 443,
            listen_address: "0.0.0.0:8080".parse().unwrap(),
            listen_address_tls: "0.0.0.0:8443".parse().unwrap(),
            listen_address_api: "0.0.0.0:8081".parse().unwrap(),
            listen_address_prom: "0.0.0.0:9090".parse().unwrap(),
            listen_address_internal: "0.0.0.0:8082".parse().unwrap(),
            listen_address_profiling: "0.0.0.0:6060".parse().unwrap(),
            plugins_directory: ::std::path::PathBuf::from("/tmp/plugins"),
            database_url: "postgres://localhost/db".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            threads: 4,
            config_directory: ::std::path::PathBuf::from("/tmp/config"),
            serve_directory: ::std::path::PathBuf::from("/tmp/serve"),
            web_app_mode: ConfigWebAppMode::SubDomain(SmolStr::new("example.com")),
            modules_internal_path: SmolStr::new("/_ksbh_internal"),
            modules_directory: ::std::path::PathBuf::from("/tmp/modules"),
            pyroscope_url: Some(ksbh_types::KsbhStr::new("http://pyroscope:4040")),
        };

        assert!(config.pyroscope_url.is_some());
        assert_eq!(
            config.pyroscope_url.unwrap().as_str(),
            "http://pyroscope:4040"
        );
    }
}
