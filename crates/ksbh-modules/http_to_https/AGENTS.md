# http_to_https Module

HTTP-to-HTTPS redirect module.

## Package

- Directory: `crates/ksbh-modules/http_to_https`
- Cargo package: `http_to_https`

## Current Entry Point

`src/lib.rs` exports:

```rust
pub fn process(
    ctx: ksbh_modules_sdk::RequestContext,
) -> Result<ksbh_modules_sdk::ModuleResult, ksbh_modules_sdk::ModuleError>
```

It is registered with:

```rust
ksbh_modules_sdk::types::ModuleType::HttpToHttps
```

## Notes

- Crate type: `cdylib`
- Built on top of `ksbh-modules-sdk`
- Redirects insecure HTTP requests with HTTP 301
- Passes WebSocket upgrade handshakes through without redirecting

## Build

```bash
cargo build -p http_to_https --manifest-path crates/Cargo.toml
```
