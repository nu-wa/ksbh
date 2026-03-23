# ksbh-config-providers-file

File-backed config provider for KSBH.

## Purpose

This crate reads YAML config from disk, applies it to the router, and watches for file changes.

## Current API

It implements the current `ksbh_core::config_provider::ConfigProvider` trait:

```rust
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn start(
        &self,
        router: RouterWriter,
        certs: CertsWriter,
        shutdown: tokio::sync::watch::Receiver<bool>,
    );
}
```

Do not document the old `get_config` / `watch_config` API here.

## YAML Shape

The current file format in `src/lib.rs` uses:

- root `Config` with `modules` and `ingresses`
- `ModuleConfig` with `name`, `type`, `weight`, `global`, optional `requires_body`, and string-string `config`
- `IngressConfig` with `host`, `tls`, `paths`, `modules`, `excluded_modules`
- `PathConfig` with `path`, `type`, `backend`, and optional `service`

## Notes

- TLS file loading from `cert_file` / `key_file` is implemented in file mode and registers certificates in the runtime cert registry.
- `tls.secret_name` in file mode is metadata only; Secret-backed TLS loading is a Kubernetes-provider feature.
- The crate uses `notify` plus `serde_yaml`.

## Build

```bash
cargo build -p ksbh-config-providers-file --manifest-path crates/Cargo.toml
```
