# ksbh

Executable entry point for the KSBH server.

## Package

- Directory: `crates/ksbh-bin`
- Cargo package: `ksbh`
- Binary: `ksbh`

## Purpose

This crate wires together the runtime:

- chooses and starts a config provider
- starts Pingora services
- configures proxy behavior, metrics, profiling, and TLS
- packages built-in app/static assets used by the host
- depends on module crates so release/runtime packaging can include them

It is not the primary implementation site for Kubernetes reconciliation or module ABI logic; those live in the config-provider crates and `ksbh-core`.

## Source Layout

Current top-level source files:

- `src/main.rs`
- `src/server.rs`
- `src/profiling.rs`
- `src/apps/`
- `src/proxy/`
- `src/services/`
- `src/tls/`

## Runtime Notes

- The file provider is selected when `KSBH__CONFIG_PATHS__CONFIG` is set.
- Otherwise the binary starts the Kubernetes config provider.
- Profiling is enabled by the default `profiling` feature.
- Release/runtime packaging includes module crates, but request-time module loading still happens through `ksbh-core`.
- Shared Askama layouts and shared inline CSS now come from `../ksbh-ui` via `askama.toml` and the `ksbh-ui` crate.
- The internal static content app supports `GET` and `HEAD`; unsupported methods on static endpoints return `405` with `Allow: GET, HEAD`.

## Services

`src/server.rs` registers the main Pingora services, including:

- config/background services
- metrics and Prometheus HTTP services
- the proxy service created by `crate::proxy::proxy_service::create_service(...)`

Check `server.rs` before documenting exact service names or ordering.

## Build

```bash
cargo build -p ksbh --manifest-path crates/Cargo.toml
```

## Verification

```bash
cargo test -p ksbh --manifest-path crates/Cargo.toml
```
