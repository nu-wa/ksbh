+++
title = "Module Result"
description = "Returning results from modules"
weight = 30
path = "/docs/module-development/module-result/"
+++

# Module Result

Every module handler returns:

```rust
::std::result::Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError>
```

## `ModuleResult`

```rust
pub enum ModuleResult {
    Pass,
    Stop(http::Response<bytes::Bytes>),
}
```

- `Pass` continues the pipeline
- `Stop(response)` returns the response immediately

## Building a stop response

```rust
let response = http::Response::builder()
    .status(http::StatusCode::FORBIDDEN)
    .body(bytes::Bytes::from("blocked"))?;

Ok(ksbh_modules_sdk::ModuleResult::Stop(response))
```

## `ModuleError`

`ModuleError` is still useful for concise failure construction:

```rust
Err(ksbh_modules_sdk::ModuleError::bad_request("invalid header"))
```

But the current `register_module!` macro behavior is important: handler errors are converted into HTTP 500 responses with the error text, not into the specific helper status code. If you need a precise HTTP status like `401`, `403`, or `429`, build a response explicitly and return `ModuleResult::Stop`.

## Practical Rule

- use `Pass` to continue
- use `Stop(response)` for any user-facing status code you care about
- use `ModuleError` for internal failures and concise control flow, knowing it currently maps to a 500
