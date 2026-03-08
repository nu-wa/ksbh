# robots-txt Module

robots.txt handling module.

## Purpose

This module handles robots.txt requests:
- Parses and serves robots.txt
- Respects crawler directives

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
cargo build -p robots-txt
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
