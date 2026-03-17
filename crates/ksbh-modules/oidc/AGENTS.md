# oidc Module

OpenID Connect authentication module.

## Purpose

This module provides OIDC authentication:
- Full OIDC authorization code flow with PKCE
- Does NOT do JWT validation directly - uses OIDC tokens via openidconnect crate
- Session cookie management with encrypted session data
- Uses MessagePack (rmp-serde) for session storage, not Redis directly

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types
- `jsonwebtoken`: JWT handling (for JWT claims parsing, not validation)
- `openidconnect`: OIDC protocol
- `reqwest`: HTTP calls to OIDC provider
- `rmp-serde`: MessagePack for session storage
- `base64`: Token encoding

## Build

```bash
cargo build -p oidc
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
