//! Bench scenario descriptors.
//!
//! One TOML file per scenario under `bench/scenarios/<id>.toml`. The file
//! describes the workload (duration, rate, connections, tls, body bytes),
//! the vegeta target lines (with `{{ var }}` placeholders), and optional
//! nginx config overrides.

use serde::{Deserialize, Serialize};

/// Top-level scenario file.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Scenario {
    pub scenario: ScenarioMeta,
    pub vegeta: VegetaSpec,
    #[serde(default)]
    pub nginx: NginxOverrides,
    #[serde(default)]
    pub module: Option<ModuleBenchSpec>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScenarioMeta {
    pub id: String,
    pub duration_s: u32,
    /// Requests per second. `0` means saturate (unbounded).
    pub rate: u32,
    pub connections: u32,
    pub tls: bool,
    pub body_bytes: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VegetaSpec {
    /// Multiline string with `{{ var }}` placeholders.
    pub targets: String,
}

/// Optional module bench spec — only used in `KsbhModule` mode.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ModuleBenchSpec {
    /// Module type string, e.g. `"oidc"` or `"proof-of-work"`.
    pub module_type: String,
    /// Optional inline config fragment for the module instance.
    #[serde(default)]
    pub config: Option<String>,
}

/// Optional nginx directives that override the default template.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NginxOverrides {
    #[serde(default)]
    pub proxy_buffer_size: Option<String>,
    #[serde(default)]
    pub proxy_buffers: Option<String>,
    #[serde(default)]
    pub keepalive_timeout: Option<String>,
    #[serde(default)]
    pub client_max_body_size: Option<String>,
    #[serde(default)]
    pub client_body_timeout: Option<String>,
    #[serde(default)]
    pub client_header_timeout: Option<String>,
    #[serde(default)]
    pub send_timeout: Option<String>,
    #[serde(default)]
    pub large_client_header_buffers: Option<String>,
}
