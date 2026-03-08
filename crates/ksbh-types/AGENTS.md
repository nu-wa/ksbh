# ksbh-types

Shared type definitions used across the KSBH workspace.

## Purpose

This crate contains common types and traits used by all other crates:
- Session trait abstractions
- Configuration types
- Protocol definitions
- Serialization primitives

## Key Dependencies

- `http`: HTTP types
- `smol_str`: Efficient string type
- `pingora`: HTTP framework types
- `serde`: Serialization

## Build

```bash
cargo build -p ksbh-types
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
