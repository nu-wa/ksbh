use crate::config_types::*;

/// Root configuration for the KSBH proxy server.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub redis_url: Option<String>,
    #[serde(default)]
    pub cookie_key: Option<String>,
    #[serde(default)]
    pub constants: ConfigDefaults,
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

impl Config {
    /// Loads configuration from YAML file and environment variables.
    ///
    /// Precedence (highest to lowest): environment variables (prefix `KSBH__`),
    /// YAML file at path specified by `KSBH__CONFIG_PATHS__CONFIG` or default
    /// `/app/config/config.yaml`.
    pub fn load() -> Result<Self, crate::config_error::ConfigError> {
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

    fn validate(&self) -> Result<(), crate::config_error::ConfigError> {
        if let Some(url) = &self.redis_url
            && url.trim().is_empty()
        {
            return Err(crate::config_error::ConfigError::ValidationError("redis_url cannot be empty"));
        }

        let cookie_key = self.cookie_key.as_ref().ok_or_else(|| {
            crate::config_error::ConfigError::MissingMandatoryValue(
                "cookie_key must be provided via config or KSBH__COOKIE_KEY".to_string(),
            )
        })?;

        if cookie_key.trim().is_empty() {
            return Err(crate::config_error::ConfigError::ValidationError("cookie_key cannot be empty"));
        }

        if crate::cookie::Key::try_from(cookie_key.as_bytes()).is_err() {
            return Err(crate::config_error::ConfigError::ValidationError(
                "cookie_key must be at least 64 bytes",
            ));
        }

        if self.constants.cookie_name.trim().is_empty() {
            return Err(crate::config_error::ConfigError::ValidationError(
                "constants.cookie_name cannot be empty",
            ));
        }

        http::header::HeaderName::from_bytes(self.constants.proxy_header_name.as_bytes())
            .map_err(|_| crate::config_error::ConfigError::ValidationError("constants.proxy_header_name is invalid"))?;

        http::HeaderValue::from_str(&self.constants.proxy_header_value)
            .map_err(|_| crate::config_error::ConfigError::ValidationError("constants.proxy_header_value is invalid"))?;

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
            constants: crate::config_types::ConfigDefaults::default(),
            pyroscope_url: None,
            ports: crate::config_types::ConfigPorts::default(),
            listen_addresses: crate::config_types::ConfigListenAddresses::default(),
            config_paths: crate::config_types::ConfigFilePaths::default(),
            url_paths: crate::config_types::ConfigURLPaths::default(),
            threads: 8,
            performance: crate::config_types::ConfigPerformance::default(),
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
