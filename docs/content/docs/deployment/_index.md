+++
title = "Deployment"
weight = 50
path = "/deployment/"
+++

# Deployment Overview

KSBH can run as a standalone binary, in a container, or through the Helm chart in `charts/ksbh/`. The most concrete deployment path in this repo today is the Helm chart plus the local `mise` orchestration tasks.

## Deployment Options

| Option | Best For | Complexity | Notes |
|--------|----------|------------|-------|
| **Standalone** | Local development | Low | Build `ksbh` from `crates/` and point it at a config source |
| **Docker** | Single-host container deployments | Medium | Use `docker/build/release.Dockerfile` or `mise run build-release-image` |
| **Kubernetes** | Cluster deployments | High | Use `charts/ksbh` and set `configProvider.mode=file` or `kubernetes` |

## Local Kubernetes Workflow

The repo ships a `mise` task named `e2e-kubernetes-provider` that:

- creates a kind cluster from the repo's kind test configuration
- builds and loads the release image
- installs the chart with `configProvider.mode=kubernetes`
- provisions fixture backends and test content
- runs the integration flow against fixed host ports

The kind mapping used by that flow is:

| Host Port | Target |
|-----------|--------|
| `18080` | NodePort HTTP |
| `18443` | NodePort HTTPS |
| `18083` | Profiling |
| `18084` | Prometheus metrics |

Use the more specific pages in this section for Docker, Kubernetes, CRDs, and production notes.
