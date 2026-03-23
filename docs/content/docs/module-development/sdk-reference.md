+++
title = "SDK Reference"
description = "Complete API reference for ksbh-modules-sdk"
weight = 20
path = "/docs/module-development/sdk-reference/"
+++

# SDK API Reference

## RequestContext

The handler receives a `ksbh_modules_sdk::RequestContext<'_>` with:

- `config`
- `headers`
- `request`
- `body`
- `session`
- `logger`
- `mod_name`
- `metrics_key`
- `cookie_header`
- `metrics`
- `internal_path`

`logger` is a field, not a method. Use `ctx.logger.info("...")` or the logging macros with `ctx.logger`.

## RequestInfo

The main request fields are:

- `uri`
- `host`
- `method`
- `path`
- `query_params`
- `scheme`
- `port`

## Session API

`ctx.session` provides:

```rust
pub fn get(&self, key: &str) -> ::std::option::Option<::std::vec::Vec<u8>>
pub fn set(&self, key: &str, data: &[u8]) -> bool
pub fn set_with_ttl(&self, key: &str, data: &[u8], ttl_secs: u64) -> bool
pub fn session_id(&self) -> [u8; 16]
```

## Metrics API

`ctx.metrics` provides:

```rust
pub fn good_boy(&self, metrics_key: &[u8]) -> bool
pub fn get_score(&self, metrics_key: &[u8]) -> u64
```

Most modules pass `ctx.metrics_key` back into those methods.

## Logger API

Examples:

```rust
ctx.logger.error("failed");
ctx.logger.warn("warning");
ctx.logger.info("info");
ctx.logger.debug("debug");

log_info!(ctx.logger, "path = {}", ctx.request.path);
```

## Request Mutability

The SDK currently does not expose a way to mutate the live downstream request. If you need to alter behavior, either:

- decide to `Pass`
- decide to `Stop(response)`

## Error Helpers

`ModuleError` provides helpers like:

- `bad_request`
- `unauthorized`
- `forbidden`
- `not_found`
- `internal_error`
- `too_many_requests`
- `critical`

Use them for concise error construction, but keep the current runtime behavior in mind: the SDK macro currently turns handler errors into HTTP 500 responses.
