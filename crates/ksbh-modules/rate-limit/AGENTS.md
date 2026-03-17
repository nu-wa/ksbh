# rate-limit Module

Request rate limiting module.

## Purpose

Metrics-based rate limiting using score thresholds. Reads `score_threshold` from config (default: 100). Uses `ctx.metrics.get_score()` to get client's current score. Returns HTTP 429 with `Retry-After` and `X-Score` headers when threshold exceeded.

## Implementation

- **Crate type**: `cdylib` (dynamic library)
- **Interface**: FFI using the ABI defined in `ksbh_core/src/modules/abi/`
- **Loaded by**: ksbh-core at runtime via dynamic library loading

## Key Dependencies

- `ksbh-core`: Core types and FFI interface
- `ksbh-types`: Shared types
- `uuid`: Client identification

## Build

```bash
cargo build -p rate-limit
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
