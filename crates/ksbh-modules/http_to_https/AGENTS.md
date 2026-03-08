# http_to_https Module

KSBH plugin module that redirects HTTP requests to HTTPS.

## Purpose

This module intercepts HTTP requests and issues a 301 redirect to the HTTPS equivalent.

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types

## Build

```bash
cargo build -p http_to_https
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
