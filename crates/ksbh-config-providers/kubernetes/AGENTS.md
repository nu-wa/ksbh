# ksbh-config-providers-kubernetes

Kubernetes-based configuration provider for KSBH.

## Purpose

This crate provides configuration loading from Kubernetes Custom Resources:
- Loads `ModuleConfiguration` CRD resources
- Watches for configuration changes via Kubernetes API
- Supports cluster-scoped configuration

## Implementation

Implements the `ConfigProvider` trait from `ksbh_core`:

```rust
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn get_config(&self) -> Result<ProxyConfig, ConfigError>;
    async fn watch_config(&self) -> Result<Receiver<ProxyConfig>, ConfigError>;
}
```

## Key Dependencies

- `ksbh-core`: Core types and ConfigProvider trait
- `kube`: Kubernetes client
- `k8s-openapi`: Kubernetes API types
- `ksbh-types`: Shared types

## Kubernetes CRD

Uses the `ModuleConfiguration` Custom Resource:

```rust
#[derive(serde::Serialize, serde::Deserialize, kube::CustomResource, schemars::JsonSchema)]
#[kube(
    group = "modules.ksbh.rs",
    version = "v1",
    kind = "ModuleConfiguration",
    namespaced = false
)]
pub struct ModuleConfigurationSpec {
    pub name: String,
    pub r#type: ModuleConfigurationType,
    // ...
}
```

## Build

```bash
cargo build -p ksbh-config-providers-kubernetes
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
