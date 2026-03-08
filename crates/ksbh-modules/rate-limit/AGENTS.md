# rate-limit Module

Request rate limiting module.

## Purpose

This module provides rate limiting functionality:
- Per-IP rate limiting
- Configurable thresholds
- Redis-backed state

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types
- `pingora`: HTTP framework

## Build

```bash
cargo build -p rate-limit
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
