# proof-of-work Module

Proof-of-work challenge module for bot mitigation.

## Package

- Directory: `crates/ksbh-modules/proof-of-work`
- Cargo package: `proof-of-work`

## Notes

- Crate type: `cdylib`
- Uses `askama`, `blake3`, `sha2`, `hex`, `serde`, `bytes`, `http`, `tracing`, and `urlencoding`
- Template assets live under `templates/`, reusable layouts come from `../../ksbh-ui/templates` via `askama.toml`, and supporting code lives in `src/templates.rs`
- Stores completion state in module session storage rather than the proxy cookie
- The module currently treats too-short or invalid secrets as deployer misconfiguration and warns/passes instead of failing closed

## Build

```bash
cargo build -p proof-of-work --manifest-path crates/Cargo.toml
```
