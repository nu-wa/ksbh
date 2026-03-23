+++
title = "Examples"
description = "Real-world module examples"
weight = 40
path = "/docs/module-development/examples/"
+++

# Example Modules

These examples stay within the behavior the current SDK actually exposes.

## Example 1: IP Blocking

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ::std::result::Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let blocked_ips = ctx
        .config
        .get("blocked_ips")
        .map(|v| v.split(',').map(|s| s.trim().to_owned()).collect::<::std::vec::Vec<_>>())
        .unwrap_or_default();

    if let Some(forwarded) = ctx.headers.get("X-Forwarded-For") {
        if let Ok(ip) = forwarded.to_str() {
            if blocked_ips.iter().any(|blocked| ip.contains(blocked)) {
                ctx.logger.info(&format!("blocking {}", ip));
                let response = http::Response::builder()
                    .status(http::StatusCode::FORBIDDEN)
                    .body(bytes::Bytes::from("blocked"))?;
                return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
            }
        }
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}
```

## Example 2: API Guard

If you need a real `401` or `403`, return a response explicitly:

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ::std::result::Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let api_key = ctx
        .headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    if api_key.is_none() {
        let response = http::Response::builder()
            .status(http::StatusCode::UNAUTHORIZED)
            .body(bytes::Bytes::from("missing api key"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}
```

## Example 3: Score-Based Guard

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ::std::result::Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let score = ctx.metrics.get_score(ctx.metrics_key);
    let threshold = ctx
        .config
        .get("block_threshold")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    if score > threshold {
        let response = http::Response::builder()
            .status(http::StatusCode::TOO_MANY_REQUESTS)
            .body(bytes::Bytes::from("too many requests"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}
```

## Configuration Shape

Use normal KSBH config, not a `proxy.modules` block:

```yaml
modules:
  - name: score-guard
    type: score-guard
    config:
      block_threshold: "100"

ingresses:
  - name: app
    host: example.com
    modules:
      - score-guard
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web
          port: 80
```
