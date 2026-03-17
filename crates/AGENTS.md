# Crates Workspace

This directory contains the Cargo workspace for the KSBH project.

## Workspace Location

**Important**: The Cargo workspace root is in `crates/`, NOT in the project root.
All cargo commands must use the `-p <crate-name>` flag to target specific crates.

## Crates

| Crate | Description |
|-------|-------------|
| `ksbh-bin` | Main binary entry point |
| `ksbh-core` | Core library with proxy logic, modules, routing, storage |
| `ksbh-modules-sdk` | SDK for building FFI plugin modules |
| `ksbh-types` | Shared type definitions |
| `ksbh-config-providers-file` | File-based configuration provider |
| `ksbh-config-providers-kubernetes` | Kubernetes-based configuration provider |
| `ksbh-modules/http_to_https` | HTTP to HTTPS redirect module |
| `ksbh-modules/oidc` | OpenID Connect authentication module |
| `ksbh-modules/proof-of-work` | PoW challenge module |
| `ksbh-modules/rate-limit` | Rate limiting module |
| `ksbh-modules/robots-txt` | robots.txt handling module |
| `tests` | Integration tests |

## Cargo Commands

Always use `-p` flag:

```bash
# Build a specific crate
cargo build -p ksbh-core

# Test a specific crate
cargo test -p ksbh-core

# Run clippy on a specific crate
cargo clippy -p ksbh-core -- -D warnings

# Build all crates
cargo build
```

## Reading This Documentation

- **Root AGENTS.md**: General conventions and project overview
- **This file**: Workspace structure and cargo commands
- **Crate-specific AGENTS.md**: Details for each crate (e.g., `ksbh-core/AGENTS.md`)
- **Module AGENTS.md**: Details for each module (e.g., `ksbh-modules/oidc/AGENTS.md`)

## Conventions

Follow the general conventions in the root `AGENTS.md`, particularly:
- No `use` imports - use full paths
- No `unwrap()`/`expect()` in production
- Return `Result` by default
- Run verification (check, clippy, fmt) before considering task complete
