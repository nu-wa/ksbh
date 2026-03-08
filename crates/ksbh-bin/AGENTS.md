# ksbh-bin

Main binary entry point for the KSBH reverse proxy server.

## Purpose

This crate is the executable entry point that:
- Starts the pingora-based HTTP/HTTPS server
- Integrates all modules (http_to_https, oidc, rate-limit, etc.)
- Handles TLS configuration
- Manages static content serving
- Provides Kubernetes integration

## Key Dependencies

- `ksbh-core`: Core library functionality
- `pingora`: HTTP server framework
- `kube`: Kubernetes client
- All module crates: http_to_https, oidc, proof-of-work, rate-limit, robots-txt

## Build

```bash
cargo build -p ksbh
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
