# ksbh-modules-sdk

SDK for building FFI modules for the KSBH reverse proxy.

## Purpose

This crate provides a convenient API for building dynamically-loaded modules:

- **Context helpers**: Safe wrappers around `ModuleContext`
- **Session access**: Methods to read/write session data
- **Logging**: Host function for module logging
- **FFI utilities**: Type-safe helpers for FFI boundary
- **Error types**: Module-specific error handling

## Relationship to ksbh-core

The SDK sits on top of the ABI defined in `ksbh_core::modules::abi`:

- `ksbh-core/abi`: Raw C-compatible types and FFI definitions
- `ksbh-modules-sdk`: Higher-level Rust API wrapping the FFI

## Using the SDK

Add as a dependency in your module:

```toml
[dependencies]
ksbh-modules-sdk = { path = "../../ksbh-modules-sdk/" }
```

## Key Components

- `context.rs`: `ModuleContext` wrapper with safe accessors
- `session.rs`: Session data read/write helpers
- `logger.rs`: Logging host function wrapper
- `types.rs`: Common types (headers, cookies, etc.)
- `result.rs`: `ModuleResult` type for error handling
- `ffi/mod.rs`: FFI boundary utilities

## Module Implementation Pattern

Use the `register_module!` macro which handles all FFI boilerplate:

```rust
fn handle_request(
    mut ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ksbh_modules_sdk::ModuleResult {
    // Access request info
    let path = ctx.request().path();
    
    // Process request and return Pass to continue, or Stop with response
    if some_condition {
        let response = http::Response::builder()
            .status(401)
            .body(bytes::Bytes::new())
            .unwrap();
        ksbh_modules_sdk::ModuleResult::Stop(response)
    } else {
        ksbh_modules_sdk::ModuleResult::Pass
    }
}

ksbh_modules_sdk::register_module!(
    handle_request, 
    ksbh_core::modules::abi::ModuleTypeCode::Custom
);
```

### Key SDK Types

- `ksbh_modules_sdk::RequestContext`: Safe wrapper around the raw module context
- `ksbh_modules_sdk::ModuleResult::Pass`: Continue request processing
- `ksbh_modules_sdk::ModuleResult::Stop(Response)`: Stop processing and return response
- `ksbh_modules_sdk::RequestInfo`: Access to request path, headers, etc.

## Build

```bash
cargo build -p ksbh-modules-sdk
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
