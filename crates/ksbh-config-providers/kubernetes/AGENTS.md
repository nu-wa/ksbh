# ksbh-config-providers-kubernetes

Kubernetes-backed config provider for KSBH.

## Purpose

This provider reconciles runtime config from Kubernetes resources.

Current responsibilities include:

- cluster-scoped `ModuleConfiguration` resources
- namespaced `Ingress` resources
- related `Service` and `Secret` watchers used during ingress reconciliation

`ModuleConfiguration.spec` includes explicit execution metadata such as `weight`; do not assume module `type` defines runtime order.
`ModuleConfiguration.spec.config` is supported for inline key/value module config. When both `spec.config` and `spec.secretRef` are set, secret values override inline keys on conflict.

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

## Important Files

- `src/lib.rs`
- `src/ingress.rs`

## Notes

- `ModuleConfiguration` is cluster-scoped.
- `Ingress`, `Service`, and `Secret` reconciliation is namespaced.
- This crate is not just a CRD watcher; ingress reconciliation is a core part of its behavior.

## Build

```bash
cargo build -p ksbh-config-providers-kubernetes --manifest-path crates/Cargo.toml
```
