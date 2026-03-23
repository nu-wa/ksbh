# ksbh-types

Shared lightweight types and traits used across the workspace.

## Current Contents

This crate is narrower than a general “all shared config/protocol types” bucket.

- `src/ksbh_str.rs` for `KsbhStr`
- `src/providers/` for proxy/provider traits
- `src/requests/` for HTTP request/response-related types
- `PublicConfig`
- `Ports`
- `ArcHashMap`
- `prelude` re-exports

## Notes

- `ArcHashMap` currently aliases `arc_swap::ArcSwap<::std::collections::HashMap<...>>`.
- The crate has a `test-util` feature.
- Do not describe this crate as owning all configuration or protocol primitives for the repo.

## Build

```bash
cargo build -p ksbh-types --manifest-path crates/Cargo.toml
```
