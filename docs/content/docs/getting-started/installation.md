+++
title = "Installing KSBH"
weight = 20
+++

# Installing KSBH

## Prerequisites

**System libraries** (Debian/Ubuntu): pkg-config, libssl-dev, build-essential, cmake

**Redis** (optional): Recommended for session storage, scoring, and modules that depend on shared state.

## Kubernetes

### Helm

```bash
helm install ksbh ./charts/ksbh --namespace ksbh --create-namespace
```

The local chart in `charts/ksbh/` installs the `ModuleConfiguration` CRD.

## Docker

## Standalone Binary

Requires [rustup](https://rust-lang.org/tools/install/) or [mise](https://mise.jdx.dev).

```bash
git clone https://github.com/nu-wa/ksbh.git
cd ksbh/crates
cargo build -p ksbh --release
# or: mise run build
```

If you are still in `crates/`, the binary is `target/release/ksbh`.

```bash
export KSBH__COOKIE_KEY="$(openssl rand -base64 64)"
export KSBH__REDIS_URL="redis://localhost:6379"
./target/release/ksbh
```
