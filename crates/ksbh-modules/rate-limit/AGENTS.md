# rate-limit Module

Score-based rate limiting module.

## Package

- Directory: `crates/ksbh-modules/rate-limit`
- Cargo package: `rate-limit`

## Current Behavior

`src/lib.rs` currently:

- reads `score_threshold` from config, defaulting to `100`
- calls `ctx.reputation_score()?`
- can use `ctx.reputation_good_boy()?` for a boolean reputation check
- returns HTTP `429` with `Retry-After` and `X-Score` when the threshold is exceeded

## Notes

- Crate type: `cdylib`
- Uses `bytes`, `http`, `tracing`, `ksbh-modules-sdk`, and `ksbh-core`

## Build

```bash
cargo build -p rate-limit --manifest-path crates/Cargo.toml
```
