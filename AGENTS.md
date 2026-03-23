# KSBH Repository

## Scope

This file covers repo-level structure, current workflows, and conventions.

- Repo root: `/home/smspl/dev/rust/ksbh`
- Rust workspace manifest: `/home/smspl/dev/rust/ksbh/crates/Cargo.toml`
- Docs site root: `/home/smspl/dev/rust/ksbh/docs`
- Helm chart root: `/home/smspl/dev/rust/ksbh/charts/ksbh`

Always read this file first, then the nearest crate- or area-specific `AGENTS.md`.

## Repo Structure

```text
ksbh/
├── crates/                 # Rust workspace
├── charts/ksbh/            # Helm chart, templates, CRDs, values
├── docs/                   # Dodeca docs site
├── docker/                 # Build and compose assets
├── mise-tasks/             # Task entrypoints used for local workflows
├── tests/                  # kind configs, k8s fixtures, Playwright e2e
└── .forgejo/workflows/     # CI workflows
```

## Rust Workspace

The Rust workspace lives under `crates/`. Common package names:

- `ksbh` in `crates/ksbh-bin`
- `ksbh-core`
- `ksbh-ui`
- `ksbh-modules-sdk`
- `ksbh-types`
- `ksbh-config-providers-file`
- `ksbh-config-providers-kubernetes`
- `http_to_https`
- `oidc`
- `proof-of-work`
- `rate-limit`
- `robots-txt`
- `tests`

Default to `cargo ... -p <package>` when targeting one package. Workspace-wide commands do not need `-p`.

## Docs

The docs site uses Dodeca plus Tailwind v4/DaisyUI.

- Source CSS: `docs/css/base.css`
- Generated CSS: `docs/static/css/style.css`
- Shared UI source CSS: `crates/ksbh-ui/static/css/shared.css`
- Shared UI generated CSS: `crates/ksbh-ui/static/css/style.css`
- One-shot docs CSS build: `cd docs && deno task build:css`
- Start CSS watcher: `cd docs && deno task dev:css`
- Build CSS once: `mise run build-css`
- Start docs server: `cd docs && ddc serve`

Do not treat `docs/static/css/style.css` as source of truth.

## Helm Chart

The deployment chart lives in `charts/ksbh`.

- Main files: `Chart.yaml`, `values.yaml`
- Templates: `charts/ksbh/templates/`
- CRDs: `charts/ksbh/crds/`
- Lint task: `mise run lint-helm-chart`

If chart structure, values, templates, or CRDs change, update the relevant `AGENTS.md`.

## E2E and Local Infra

Repo-level infra and e2e helpers live outside the Rust workspace:

- `tests/kind/` for kind cluster config
- `tests/k8s/` for Kubernetes fixtures
- `tests/playwright/` for browser-driven e2e
- `mise-tasks/` for local orchestration
- `.forgejo/workflows/` for CI workflows, including separate Kubernetes and direct-binary e2e entries

Notable tasks:

- `mise run build-css`
- `mise run run-kind`
- `mise run clean-kind`
- `mise run build-release-image`
- `mise run build-docs-site`
- `mise run build-helm-repo`
- `mise run build-docs-site-image`
- `mise run build-charts-site-image`
- `mise run lint-helm-chart`
- `mise run package-helm-chart`
- `mise run e2e`
- `mise run e2e-binary`
- `mise run dynamic-modules-smoke`
- `mise run miri-modules-sdk-ffi`
- `mise run e2e-file-provider`
- `mise run e2e-kubernetes-provider`

`e2e-kubernetes-provider` does more than run Rust tests: it provisions kind resources, loads the release image, installs the Helm chart, applies fixtures, and runs ignored Rust e2e binaries plus the Playwright browser PoW test.
`e2e-binary` builds `ksbh` and runs direct-binary integration tests that spawn `./ksbh` with file-provider config and per-test loopback ports.
`dynamic-modules-smoke` builds the test-only `dynamic-ffi-smoke` `cdylib` under `crates/ksbh-modules/test-modules/` and runs the native `ksbh-core` integration test that loads it through `ModuleHost`. It accepts `KSBH_DYNAMIC_SMOKE_LOOPS` to increase repeated real `.so` call/free cycles in CI.
`miri-modules-sdk-ffi` builds a dedicated nightly+Miri Docker image and runs the in-process `ksbh-modules-sdk` FFI smoke test. That suite now covers custom-type export stability, pass/error response conversion, and repeated response allocation/free loops. It does not attempt to run the compiled `ksbh` binary or dynamically loaded module `.so` files under Miri.

Current Forgejo workflows:

- `.forgejo/workflows/ci.yaml`
  - `e2e-binary`
  - `modules-memory-check`
  - `e2e-kubernetes`
  - `docker-build-publish`
  - `helm-chart-artifacts`

Local Forgejo composite actions:

- `.forgejo/actions/setup-mise/` for repeated apt+misesetup
- `.forgejo/actions/run-mise-task/` for repeated `mise install` + task execution
- `.forgejo/actions/install-playwright/` for the browser setup used by Kubernetes e2e

## Verification Rules

Scope verification to the type of change:

- Rust code changes:
  - `cargo fmt --all --manifest-path crates/Cargo.toml`
  - `cargo check --manifest-path crates/Cargo.toml`
  - `cargo clippy --manifest-path crates/Cargo.toml --all-targets -- -D warnings`
  - relevant `cargo test` commands
- Docs template/CSS/content changes:
  - use `ddc serve` and the CSS watcher
  - do not run Cargo just for docs-only edits
- Helm chart changes:
  - `mise run lint-helm-chart`
- AGENTS/docs-only maintenance:
  - no Rust build required unless you also changed Rust code

## Engineering Conventions

- Do not use Rust `use` imports in workspace code. Use full paths.
- Do not leave `.unwrap()` or `.expect()` in production code.
- Prefer returning `Result` from non-trivial public functions.
- Search the existing codebase before inventing new patterns.
- Use `rg`/`find` to locate current implementations before updating docs or instructions.

## Documentation Hygiene

When changing project structure, commands, chart behavior, docs workflow, SDK APIs, config-provider APIs, or test workflows, update the relevant `AGENTS.md` files in the same task.
