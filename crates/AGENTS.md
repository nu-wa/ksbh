# Crates Workspace

This directory contains the Rust workspace for KSBH.

## Manifest

- Workspace root: `/home/smspl/dev/rust/ksbh/crates`
- Workspace manifest: `/home/smspl/dev/rust/ksbh/crates/Cargo.toml`

## Members

Directory paths and package names are not always the same.

| Directory | Cargo package |
| --- | --- |
| `ksbh-bin` | `ksbh` |
| `ksbh-core` | `ksbh-core` |
| `ksbh-ui` | `ksbh-ui` |
| `ksbh-modules-sdk` | `ksbh-modules-sdk` |
| `ksbh-types` | `ksbh-types` |
| `ksbh-config-providers/file` | `ksbh-config-providers-file` |
| `ksbh-config-providers/kubernetes` | `ksbh-config-providers-kubernetes` |
| `ksbh-modules/http_to_https` | `http_to_https` |
| `ksbh-modules/oidc` | `oidc` |
| `ksbh-modules/proof-of-work` | `proof-of-work` |
| `ksbh-modules/rate-limit` | `rate-limit` |
| `ksbh-modules/robots-txt` | `robots-txt` |
| `tests` | `tests` |

## Cargo Usage

Use `-p <package>` by default when targeting one crate:

```bash
cargo build -p ksbh-core --manifest-path crates/Cargo.toml
cargo test -p ksbh --manifest-path crates/Cargo.toml
cargo clippy -p ksbh-core --manifest-path crates/Cargo.toml -- -D warnings
```

Workspace-wide commands are fine when appropriate:

```bash
cargo build --manifest-path crates/Cargo.toml
cargo fmt --all --manifest-path crates/Cargo.toml
```

## Reading Order

- Root `AGENTS.md` for repo-wide workflow
- This file for workspace/package naming
- Crate-local `AGENTS.md` for implementation details
- Module-local `AGENTS.md` under `ksbh-modules/*` when working on a module crate

## Conventions

Follow the root conventions, especially:

- full-path Rust references instead of `use`
- no production `unwrap()`/`expect()`
- verification only when the change type requires it
