# robots-txt Module

robots.txt handling module.

## Purpose

Serves static robots.txt content from configuration. Simply returns the `content` field from module config as `text/plain` when path is `/robots.txt` and method is GET. Does NOT parse or respect crawler directives.

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types

## Build

```bash
cargo build -p robots-txt
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
