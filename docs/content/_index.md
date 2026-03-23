+++
title = "KSBH"
description = "A Rust reverse proxy with pluggable request modules and file or Kubernetes-driven configuration"
+++

## Why This Exists

- Explore how a configurable proxy runtime fits together in practice
- Experiment with dynamic request modules and execution ordering
- Compare file-based and Kubernetes-native configuration models
- Learn by building a cohesive project instead of isolated examples

## Request Flow

```aasvg
┌──────────┐
│ Request  │
└────┬─────┘
     │
     ▼
┌──────────────┐
│ Route Match  │
│ host + path  │
└────┬─────────┘
     │
     ▼
┌──────────────────────────────┐
│ Global Modules               │
│ sorted by weight descending  │
└────┬─────────────────────────┘
     │
     ▼
┌──────────────────────────────┐
│ Ingress Modules              │
│ sorted by weight descending  │
└────┬─────────────────────────┘
     │
     ├─────────────── module replies ───────────────▶ response
     │
     ▼
┌──────────────┐
│ Backend      │
└──────────────┘
```

## Core Pieces

### Runtime

The runtime handles routing, module execution, proxying, cookie and session integration, and runtime state updates.

### Modules

Modules are dynamically loaded request filters. They can inspect the request, use module configuration and session state, and either continue processing or return a response directly.

Built-in modules cover common edge concerns:

- HTTP to HTTPS redirects
- `robots.txt` responses
- OpenID Connect authentication
- proof-of-work challenges
- rate limiting

### Configuration Providers

KSBH supports two primary configuration sources:

- file-based configuration for local or direct-binary deployments
- Kubernetes resources for cluster-native deployments

Both feed the same runtime model: module definitions, ingress attachment, routing, and backend targets.

## Documentation

- [Getting Started](/docs/getting-started/) for installation, first run, and local workflows
- [Configuration](/docs/configuration/) for runtime options, environment variables, and providers
- [Modules](/docs/modules/) for built-in modules and module configuration
- [Deployment](/docs/deployment/) for Docker, Kubernetes, CRDs, and production notes
- [Module Development](/docs/module-development/) for writing custom modules

## Start Here

If you want to see how the project is put together, start with [Getting Started](/docs/getting-started/).

If you already know how you want to run it:

- use [Configuration Providers](/docs/configuration/providers/) to choose file or Kubernetes config
- use [Modules](/docs/modules/) to understand request processing behavior
- use [Kubernetes Deployment](/docs/deployment/kubernetes/) if you are targeting the Helm chart
