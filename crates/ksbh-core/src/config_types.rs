#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigDefaults {
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

impl Default for ConfigDefaults {
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
