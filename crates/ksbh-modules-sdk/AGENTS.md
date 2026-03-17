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
- `error.rs`: Error types and helper constructors
- `metrics.rs`: Metrics reporting via host functions
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

### Error Handling

The SDK provides `ModuleError` for error handling with convenience constructors:

```rust
use ksbh_modules_sdk::ModuleError;

// Return 400 Bad Request
return Err(ModuleError::bad_request("Invalid request"));

// Return 401 Unauthorized
return Err(ModuleError::unauthorized("Missing credentials"));

// Return 403 Forbidden
return Err(ModuleError::forbidden("Access denied"));

// Return 404 Not Found
return Err(ModuleError::not_found("Resource not found"));

// Return 500 Internal Server Error
return Err(ModuleError::internal_error("Something went wrong"));
```

Available constructors:
- `ModuleError::bad_request(msg: &str) -> Self`
- `ModuleError::unauthorized(msg: &str) -> Self`
- `ModuleError::forbidden(msg: &str) -> Self`
- `ModuleError::not_found(msg: &str) -> Self`
- `ModuleError::internal(msg: &str) -> Self` (alias for `internal_error`)
- `ModuleError::too_many_requests(msg: &str) -> Self`
- `ModuleError::critical(err: E) -> Self` for critical errors

### Metrics

The SDK provides `MetricsHandle` for reporting metrics to the host:

```rust
let ctx: ksbh_modules_sdk::RequestContext = /* ... */;

// Reduce score by 50 (for good behavior like completing a challenge)
let success = ctx.metrics().good_boy(b"challenge:completed");

// Get current score
let score = ctx.metrics().get_score(b"user:123");
```

Methods:
- `metrics.good_boy(metrics_key: &[u8]) -> bool` - Reduce score by 50, returns true if successful
- `metrics.get_score(metrics_key: &[u8]) -> u64` - Get current score for the key

### Logging Macros

Convenience macros for logging via the host:

```rust
log_error!(ctx.logger(), "Failed to process request: {}", error);
log_warn!(ctx.logger(), "Rate limit approaching: {}", current);
log_info!(ctx.logger(), "Request processed: {}", path);
log_debug!(ctx.logger(), "Debug info: {:?}", data);
```

Available macros:
- `log_error!(logger, message)` - Log at error level
- `log_warn!(logger, message)` - Log at warning level
- `log_info!(logger, message)` - Log at info level
- `log_debug!(logger, message)` - Log at debug level

### FFI Functions

The SDK exports the following FFI functions that the host calls:

- `free_response` - FFI function for freeing responses allocated by modules. The SDK manages memory internally, so this is currently a no-op but must be exported for the host to call.

## Build

```bash
cargo build -p ksbh-modules-sdk
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
