+++
title = "Configuration"
description = "Complete guide to configuring KSBH reverse proxy"
weight = 30
+++

# Configuration

## Quick Start

### Minimum Required Configuration

KSBH requires:

1. **Cookie encryption key** - For secure sessions
2. **A configuration source** - File provider when `KSBH__CONFIG_PATHS__CONFIG` is set, otherwise Kubernetes provider

```bash
# Minimum environment variables
export KSBH__COOKIE_KEY="your-256-bit-secret-key"
```

### Using YAML Configuration

Create a configuration file:

```yaml
# /app/config/config.yaml
modules:
  - name: "rate-limiter"
    type: "ratelimit"
    weight: 100
    global: true
    config:
      score_threshold: "100"

ingresses:
  - name: "example"
    host: "example.com"
    paths:
      - path: "/"
        type: "prefix"
        backend: "service"
        service:
          name: "web"
          port: 80
```

Set the config path:

```bash
export KSBH__CONFIG_PATHS__CONFIG="/app/config/config.yaml"
```

---

## Configuration Methods

KSBH supports multiple configuration methods that can be combined:

| Method | Use Case | Priority |
|--------|----------|----------|
| Environment variables | Runtime settings, secrets | Highest |
| YAML configuration file | Declarative routing | Medium |
| Kubernetes CRDs | Kubernetes-native | Low |
| Default values | Fallback | Lowest |

## Core Concepts

- **Modules**: Request processing plugins (rate-limit, http-to-https, oidc, pow, robots.txt, custom)
- **Ingresses**: Routing rules mapping hostnames/paths to backends
- **Backends**: `service` (Kubernetes Service), `static` (static content)

---

## Configuration Provider

KSBH uses pluggable configuration providers:

- **[File-based provider](/docs/configuration/providers/)**: Reads YAML files with hot reload
- **[Kubernetes provider](/docs/configuration/providers/)**: Watches CRDs and Ingress resources when the file path is unset

The provider is automatically selected:
- File provider: when `KSBH__CONFIG_PATHS__CONFIG` is set
- Kubernetes provider: when the variable is not set

---

## Next Steps

- Review the [Configuration Reference](/docs/configuration/reference/) for all options
- Set up [Environment Variables](/docs/configuration/env-variables/) including secrets
- Create your [YAML Configuration](/docs/configuration/yaml-config/)
- Choose and configure a [Configuration Provider](/docs/configuration/providers/)

---
