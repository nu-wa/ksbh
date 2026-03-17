# ksbh-config-providers-file

File-based configuration provider for KSBH.

## Purpose

This crate provides configuration loading from YAML files:
- Watches YAML configuration files for changes
- Hot-reloads configuration without restart
- Uses `notify` crate for file system watching

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
- `notify`: File system watching (v6 with macos_kqueue feature)
- `serde_yaml`: YAML parsing
- `ksbh-types`: Shared types

## Configuration File Format

YAML configuration files contain proxy configuration:
- Upstream definitions
- Module configurations
- Routing rules

## Build

```bash
cargo build -p ksbh-config-providers-file
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
