# oidc Module

OpenID Connect authentication module.

## Package

- Directory: `crates/ksbh-modules/oidc`
- Cargo package: `oidc`

## Notes

- Crate type: `cdylib`
- Uses `openidconnect`, `reqwest`, `jsonwebtoken`, `rmp-serde`, `serde`, and `scc`
- Uses blocking `reqwest` / `openidconnect` clients
- Stores module state in namespaced session storage, not in the host-owned proxy cookie
- Caches provider metadata and related auth state inside the module runtime
- Avoid claiming broad JWT validation behavior unless you verified it in source

## Build

```bash
cargo build -p oidc --manifest-path crates/Cargo.toml
```
