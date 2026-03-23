# rate-limit Module

Score-based rate limiting module.

## Package

- Directory: `crates/ksbh-modules/rate-limit`
- Cargo package: `rate-limit`

## Current Behavior

`src/lib.rs` currently:

- reads `score_threshold` from config, defaulting to `100`
- uses `ctx.metrics_key`
- calls `ctx.metrics.get_score(metrics_key)`
- returns HTTP `429` with `Retry-After` and `X-Score` when the threshold is exceeded

## Notes

- Crate type: `cdylib`
- Uses `bytes`, `http`, `tracing`, `ksbh-modules-sdk`, and `ksbh-core`

## Build

```bash
cargo build -p rate-limit --manifest-path crates/Cargo.toml
```
