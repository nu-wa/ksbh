# ksbh-core

Core runtime library for KSBH.

## Purpose

This crate owns the main host/runtime abstractions:

- module ABI and dynamic module registry
- router and ingress/module state
- certificate registry and TLS-facing data
- config-provider trait and background service wrapper
- proxy/runtime metrics
- storage helpers such as `RedisHashMap`
- proxy cookie handling

## Key Areas

- `src/certs/`
- `src/config/`
- `src/config_provider/`
- `src/constants.rs`
- `src/cookies/`
- `src/metrics/`
- `src/modules/`
- `src/modules/abi/`
- `src/modules/registry.rs`
- `src/proxy/`
- `src/routing/`
- `src/storage/`
- `src/utils/`
- `src/bin/generate_crd.rs`

## Config Provider API

The current trait is:

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

Do not document the old `get_config` / `watch_config` model here.

## Module System

Modules are loaded as `cdylib` libraries through the registry/ABI layer.

- Raw ABI lives in `src/modules/abi/`
- Runtime loading lives in `src/modules/registry.rs`
- The SDK macro generates `get_module_type` and `request_filter` for normal module crates
- The host-side native dynamic-module smoke test lives at `tests/module_host_dynamic.rs` and loads the `dynamic-ffi-smoke` test `cdylib` through `ModuleHost`
- That test intentionally uses one shared `ModuleHost` because ABI host callbacks are registered through process-global `OnceLock`s; keep that in mind when extending the suite

## Useful Notes

- `RedisHashMap` is the preferred Redis-backed hot/cold cache helper for module state.
- Proxy cookie ownership lives in the host; modules should store their own state in namespaced session storage instead.
- Runtime router-state metrics are emitted from in-memory router snapshots, not provider-specific event streams.

## Build

```bash
cargo build -p ksbh-core --manifest-path crates/Cargo.toml
cargo test -p ksbh-core --manifest-path crates/Cargo.toml
cargo run -p ksbh-core --bin generate_crd --manifest-path crates/Cargo.toml
```
