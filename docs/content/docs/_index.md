+++
title = "Documentation"
description = "Guides and reference material for running, configuring, and extending KSBH"
+++

# KSBH Documentation

Use this section as the guided reference for running, configuring, deploying, and extending KSBH.

## Start Here

- [Getting Started](/docs/getting-started/) for installation, quick setup, and first-run guidance
- [Configuration](/docs/configuration/) for runtime defaults, YAML structure, providers, and environment variables
- [Modules](/docs/modules/) for request filters, ordering rules, and module-specific behavior

## Sections

### [Getting Started](/docs/getting-started/)

Install and run KSBH locally, in containers, or from source.

- [Quick Start](/docs/getting-started/quick-start/) - fastest path to a running instance
- [Installation](/docs/getting-started/installation/) - build from source or use containers
- [Running](/docs/getting-started/running/) - runtime commands and invocation examples

### [Configuration](/docs/configuration/)

Provider selection, YAML structure, runtime defaults, and environment variables.

- [Reference](/docs/configuration/reference/) - complete runtime configuration reference
- [Environment Variables](/docs/configuration/env-variables/) - process-level settings and defaults
- [YAML Config](/docs/configuration/yaml-config/) - file-provider structure and examples
- [Providers](/docs/configuration/providers/) - file vs Kubernetes provider behavior

### [Modules](/docs/modules/)

Built-in request filters, ordering, scope, and attachment rules.

- [Module Configuration](/docs/modules/configuration/) - weights, scopes, exclusions, and attachment
- [Rate Limit](/docs/modules/rate-limit/) - score-based request throttling
- [Proof of Work](/docs/modules/proof-of-work/) - computational challenge flow
- [OIDC](/docs/modules/oidc/) - OpenID Connect authentication
- [Robots.txt](/docs/modules/robots-txt/) - custom crawler policy responses
- [HTTP to HTTPS](/docs/modules/http-to-https/) - redirect handling

### [Deployment](/docs/deployment/)

Docker, Kubernetes, CRDs, and production-oriented caveats.

- [Docker](/docs/deployment/docker/) - release image usage and container layout
- [Kubernetes](/docs/deployment/kubernetes/) - Helm chart deployment modes
- [CRDs](/docs/deployment/crds/) - `ModuleConfiguration` resources and ingress integration
- [Production](/docs/deployment/production/) - deployment checklist and operational caveats

### [Module Development](/docs/module-development/)

Build custom FFI modules and understand the SDK surface.

- [Getting Started](/docs/module-development/getting-started/) - create your first module
- [SDK Reference](/docs/module-development/sdk-reference/) - SDK API and host interaction
- [Module Result](/docs/module-development/module-result/) - pass/stop/error types
- [Examples](/docs/module-development/examples/) - reference implementations
