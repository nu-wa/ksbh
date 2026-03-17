# proof-of-work Module

Proof-of-work challenge module for bot mitigation.

## Purpose

This module implements PoW challenges:
- Hash-based challenge generation
- Client-side proof verification
- Anti-bot protection

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types
- `blake3`: Hashing
- `sha2`: Hashing
- `askama`: Template rendering
- `urlencoding`: URL encoding
- `uuid`: Session ID generation
- `hex`: Hex encoding

## Build

```bash
cargo build -p proof-of-work
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
