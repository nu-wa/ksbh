+++
title = "Module Development"
description = "Build custom FFI modules for the KSBH reverse proxy"
weight = 60
path = "/docs/module-development/"
+++

# Module Development

KSBH supports dynamically loaded Rust modules through a shared-library FFI boundary. Custom modules are a good fit for request filtering, custom auth gates, scoring logic, and short-circuit responses.

## What Custom Modules Can Do

- inspect request metadata, headers, body, config, and session state
- read and write session data
- inspect and adjust client score through the metrics handle
- stop the request early with a custom HTTP response
- pass the request through unchanged

## Important Constraint

The current SDK does not provide a host channel for mutating the downstream request in place. `ctx.headers` is a local copy, and `ModuleResult` only supports `Pass` or `Stop(response)`.

## SDK Overview

The `ksbh-modules-sdk` crate provides:

- `RequestContext`
- `ModuleResult`
- `ModuleError`
- session, metrics, and logging handles
- the `register_module!` macro for exporting the FFI entry points

## Quick Example

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ::std::result::Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    if ctx.request.path == "/forbidden" {
        let response = http::Response::builder()
            .status(http::StatusCode::FORBIDDEN)
            .body(bytes::Bytes::from("blocked"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::register_module!(
    process,
    ksbh_modules_sdk::types::ModuleType::Custom("example".into())
);
```

## Next Steps

- [Getting Started](@/docs/module-development/getting-started.md)
- [SDK Reference](@/docs/module-development/sdk-reference.md)
- [Module Result](@/docs/module-development/module-result.md)
- [Examples](@/docs/module-development/examples.md)
