# ksbh-modules-sdk

High-level SDK for writing KSBH FFI modules.

## Purpose

This crate wraps the raw ABI from `ksbh-core/src/modules/abi/` with a Rust-facing API for module authors.

## Source Layout

- `src/context.rs`
- `src/error.rs`
- `src/ffi/mod.rs`
- `src/logger.rs`
- `src/metrics.rs`
- `src/result.rs`
- `src/session.rs`
- `src/types.rs`

## Public Surface

Primary exports:

- `ksbh_modules_sdk::RequestContext`
- `ksbh_modules_sdk::RequestInfo`
- `ksbh_modules_sdk::ModuleResult`
- `ksbh_modules_sdk::ModuleError`
- `ksbh_modules_sdk::MetricsHandle`
- `ksbh_modules_sdk::is_websocket_upgrade_request(headers)`

## Module Pattern

Normal modules implement a handler with this shape:

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::register_module!(
    process,
    ksbh_modules_sdk::types::ModuleType::HttpToHttps
);
```

The macro exports:

- `get_module_type`
- `request_filter`

The crate also exports `free_response`.

## Error Helpers

Useful constructors from `src/error.rs`:

- `ModuleError::bad_request(...)`
- `ModuleError::unauthorized(...)`
- `ModuleError::forbidden(...)`
- `ModuleError::not_found(...)`
- `ModuleError::internal_error(...)`
- `ModuleError::too_many_requests(...)`
- `ModuleError::critical(...)`

These accept `impl Into<String>` for message-based constructors.

## Metrics

`RequestContext` exposes a `metrics` field of type `MetricsHandle`.

Current methods:

- `ctx.metrics.good_boy(metrics_key)`
- `ctx.metrics.get_score(metrics_key)`

Both require a metrics key argument.

## Notes

- The SDK examples in this file should avoid `unwrap()` in production-style snippets.
- Prefer documenting the current public fields and helpers from `RequestContext`, not imaginary accessor-heavy APIs.
- `register_module!` now keeps `ModuleType::Custom(...)` names in stable storage for `get_module_type()`, so dynamically loaded custom modules do not return dangling pointers to temporary strings.

## Build

```bash
cargo build -p ksbh-modules-sdk --manifest-path crates/Cargo.toml
```

## FFI Smoke Testing

- The crate now has an in-process FFI smoke test at `tests/ffi_miri.rs`.
- That test exercises:
  - `register_module!`
  - `request_filter`
  - `convert_context`
  - session callbacks
  - metrics callbacks
  - response allocation
- It also covers custom-type export stability, `Pass`, error-to-response conversion, and repeated allocation/free cycles.
- It is designed to run under Miri through `mise run miri-modules-sdk-ffi`.
- This is the intended Miri target for SDK/ABI coverage; do not describe the release-image dynamic `.so` loading path as Miri-covered.
