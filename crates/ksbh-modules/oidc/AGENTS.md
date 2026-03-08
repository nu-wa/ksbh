# oidc Module

OpenID Connect authentication module.

## Purpose

This module provides OIDC authentication:
- OAuth2/OIDC protocol implementation
- JWT token validation
- Session management
- User authentication flow

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types
- `jsonwebtoken`: JWT handling
- `openidconnect`: OIDC protocol

## Build

```bash
cargo build -p oidc
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
