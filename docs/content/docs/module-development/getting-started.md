+++
title = "Getting Started"
description = "Create your first custom module"
weight = 10
path = "/docs/module-development/getting-started/"
+++

# Getting Started with Custom Modules

## 1. Create the crate

Create a new module under `crates/ksbh-modules/`:

```bash
mkdir -p crates/ksbh-modules/my_module/src
```

Add it to the workspace members in `crates/Cargo.toml` before trying to build it with `cargo -p`.

## 2. Create `Cargo.toml`

```toml
[package]
name = "my_module"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
bytes = { workspace = true }
http = { workspace = true }
ksbh-modules-sdk = { path = "../../ksbh-modules-sdk/" }
```

## 3. Implement the module

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ::std::result::Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError> {
    let path = ctx.request.path.as_str();
    ctx.logger.info(&format!("processing {}", path));

    if ctx.config.get("block_requests").map(|v| v.as_str()) == Some("true") {
        let response = http::Response::builder()
            .status(http::StatusCode::FORBIDDEN)
            .body(bytes::Bytes::from("requests blocked by module"))?;
        return Ok(ksbh_modules_sdk::ModuleResult::Stop(response));
    }

    Ok(ksbh_modules_sdk::ModuleResult::Pass)
}

ksbh_modules_sdk::register_module!(
    process,
    ksbh_modules_sdk::types::ModuleType::Custom("my_module".into())
);
```

## 4. Build it

```bash
cd crates
cargo build -p my_module
```

## 5. Reference it from config

```yaml
modules:
  - name: my-module
    type: my_module
    global: false
    config:
      block_requests: "false"

ingresses:
  - name: example
    host: example.com
    modules:
      - my-module
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web
          port: 80
```

The module config arrives in `ctx.config` as string key-value pairs.

For custom modules, the configured `type` must match the custom module type you register in `register_module!`.
