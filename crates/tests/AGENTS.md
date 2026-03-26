# tests

Integration and e2e test crate for KSBH.

## Package

- Directory: `crates/tests`
- Cargo package: `tests`
- Edition: `2021`

## Layout

- Shared crate code: `src/`
- Ignored integration/e2e binaries: `tests/tests/`
- Shared e2e helpers: `tests/tests/common/mod.rs`
- Direct-binary harness helpers: `src/binary.rs`

Current e2e test binaries:

- `binary_baseline_e2e`
- `binary_static_content_e2e`
- `k8s_baseline_e2e`
- `k8s_ingress_lifecycle_e2e`
- `k8s_http_to_https_e2e`
- `k8s_pow_http_e2e`
- `k8s_robots_txt_e2e`
- `k8s_static_content_e2e`
- `k8s_trusted_proxies_e2e`
- `k8s_websocket_e2e`

## Common Helpers

The current reusable helpers in `tests/tests/common/mod.rs` include:

- `E2eConfig`
- `WaitRetrySettings`
- `build_http_client`
- `kube_client`
- `unique_name`
- `unique_host`
- `wait_for_host_status`
- `wait_for_host_body`
- `wait_for_metrics_ready`

Prefer these helpers over documenting stale mock-only examples.

## Notes

- The crate uses `ksbh-core` and `ksbh-types` with `test-util` features.
- Browser-driven PoW coverage lives at repo level in `tests/playwright/`; mention it as repo-level e2e, not crate-local code.
- Some tests require a Kubernetes cluster and ignored e2e flow, but not every possible `cargo test -p tests` invocation has the same runtime requirements.
- Direct-binary suites spawn the built `ksbh` binary directly in file-provider mode; the local wrapper is `mise run e2e-binary`.

## Build And Run

```bash
cargo test -p tests --manifest-path crates/Cargo.toml
cargo test -p tests --test k8s_ingress_lifecycle_e2e --manifest-path crates/Cargo.toml -- --ignored --nocapture --test-threads=1
cargo test -p tests --test binary_baseline_e2e --test binary_static_content_e2e --manifest-path crates/Cargo.toml -- --nocapture --test-threads=1
```
