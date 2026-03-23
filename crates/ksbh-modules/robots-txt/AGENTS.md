# robots-txt Module

robots.txt serving module.

## Package

- Directory: `crates/ksbh-modules/robots-txt`
- Cargo package: `robots-txt`

## Current Behavior

`src/lib.rs` currently:

- matches `GET /robots.txt`
- reads `content` from module config
- returns `text/plain`
- otherwise returns `Pass`

It serves configured content; it does not interpret crawler directives itself.

## Notes

- Crate type: `cdylib`
- Uses `bytes`, `http`, `tracing`, `ksbh-modules-sdk`, and `ksbh-core`

## Build

```bash
cargo build -p robots-txt --manifest-path crates/Cargo.toml
```
