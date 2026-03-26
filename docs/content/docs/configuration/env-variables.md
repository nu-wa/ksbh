+++
title = "Environment Variables"
description = "Complete guide to KSBH environment variables"
weight = 30
+++

# Environment Variables

KSBH uses `KSBH__...` environment variables for runtime configuration. In the current tree, the cookie key is loaded from `KSBH__COOKIE_KEY`.

---

## General Configuration

### Main Settings

| Variable | Required | Description |
|----------|----------|-------------|
| `KSBH__REDIS_URL` | No | Redis connection URL (optional; if not set, Redis-dependent features are disabled) |
| `KSBH__PYROSCOPE_URL` | No | Pyroscope profiling server URL |
| `KSBH__THREADS` | No | Number of worker threads (default: 8) |
| `KSBH__TRUSTED_PROXIES__0`, `KSBH__TRUSTED_PROXIES__1`, ... | No | Trusted proxy IP/CIDR entries used to allow `X-Forwarded-*` and `Forwarded` |

### Port Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `KSBH__PORTS__APP__HTTP` | 8080 | Internal HTTP port |
| `KSBH__PORTS__APP__HTTPS` | 8081 | Internal HTTPS port |
| `KSBH__PORTS__EXTERNAL__HTTP` | 80 | External/public HTTP port |
| `KSBH__PORTS__EXTERNAL__HTTPS` | 443 | External/public HTTPS port |

### Listen Addresses

| Variable | Default | Description |
|----------|---------|-------------|
| `KSBH__LISTEN_ADDRESSES__HTTP` | 0.0.0.0:8080 | HTTP listener address |
| `KSBH__LISTEN_ADDRESSES__HTTPS` | 0.0.0.0:8081 | HTTPS listener address |
| `KSBH__LISTEN_ADDRESSES__INTERNAL` | 0.0.0.0:8082 | Internal interface address |
| `KSBH__LISTEN_ADDRESSES__PROFILING` | 0.0.0.0:8083 | Profiling endpoint address |
| `KSBH__LISTEN_ADDRESSES__PROMETHEUS` | 0.0.0.0:8084 | Prometheus metrics address |

### File Paths

| Variable | Default | Description |
|----------|---------|-------------|
| `KSBH__CONFIG_PATHS__CONFIG` | unset at process level | Runtime path to the YAML config file. When set, the file provider is used. |
| `KSBH__CONFIG_PATHS__MODULES` | /app/modules | Dynamic module libraries |
| `KSBH__CONFIG_PATHS__STATIC_CONTENT` | /app/data/static | Static content directory |
| `KSBH__URL_PATHS__MODULES` | /_ksbh_internal/ | Module internal endpoints |

### Performance Tuning

| Variable | Default | Description |
|----------|---------|-------------|
| `KSBH__PERFORMANCE__TCP_FASTOPEN` | 12 | TCP Fast Open queue size |
| `KSBH__PERFORMANCE__SO_REUSEPORT` | null | Enable SO_REUSEPORT |
| `KSBH__PERFORMANCE__TCP_KEEPALIVE` | null | Enable TCP keep-alive |

### Constants

| Variable | Default | Description |
|----------|---------|-------------|
| `KSBH__CONSTANTS__TCP_FASTOPEN_QUEUE_SIZE` | 12 | TCP FastOpen queue size |
| `KSBH__CONSTANTS__COOKIE_NAME` | ksbh | Session cookie name |
| `KSBH__CONSTANTS__COOKIE_SECURE` | true | Mark the session cookie as `Secure` |
| `KSBH__CONSTANTS__PROXY_HEADER_NAME` | Server | Header name for proxy identification |
| `KSBH__CONSTANTS__PROXY_HEADER_VALUE` | ksbh | Header value for proxy identification |

---

## Security Variables

These variables contain sensitive data and should be protected accordingly.

### Cookie Encryption

| Variable | Required | Description |
|----------|----------|-------------|
| `KSBH__COOKIE_KEY` | Yes* | Secret key for encrypting session cookies |

```bash
export KSBH__COOKIE_KEY="your-secret-key-at-least-64-bytes-long"
```

**Note**: The key must be at least 64 bytes.

### Trusted Proxy Headers

Forwarded headers are only trusted when the TCP peer address matches one of the
configured `trusted_proxies` entries.

```bash
export KSBH__TRUSTED_PROXIES__0="10.15.0.12"
export KSBH__TRUSTED_PROXIES__1="10.15.0.0/16"
```

### Session Cookie Name

---

## Configuration Provider Selection

| Variable | Description |
|----------|-------------|
| `KSBH__CONFIG_PATHS__CONFIG` | Path to YAML config file. If set, uses file-based provider. If not set, uses Kubernetes provider. |

---

## Priority Order

| Priority | Source |
|----------|--------|
| 1 | Direct environment variable |
| 2 | YAML configuration |
| 3 | Default values |

---

## Kubernetes Secrets

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: ksbh-secrets
type: Opaque
stringData:
  cookie-key: "your-cookie-secret-here"
```

The Helm chart wires that secret into `KSBH__COOKIE_KEY`.

---

## Complete Example

```bash
#!/bin/bash
# ksbh-env.sh

# Required
export KSBH__COOKIE_KEY="your-secret-key-at-least-64-bytes-long"

# Optional
export KSBH__REDIS_URL="redis://redis.example.com:6379"
export KSBH__TRUSTED_PROXIES__0="10.15.0.0/16"

# File-based config provider
export KSBH__CONFIG_PATHS__CONFIG="/app/config/config.yaml"

# Performance tuning
export KSBH__PERFORMANCE__TCP_FASTOPEN="12"
export KSBH__PERFORMANCE__TCP_KEEPALIVE="true"

# Custom ports
export KSBH__PORTS__EXTERNAL__HTTPS="443"
```

---

## Troubleshooting

| Error | Fix |
|-------|-----|
| `KSBH__COOKIE_KEY is empty!` | Set `KSBH__COOKIE_KEY` |
| Redis-backed features do not work | Set `KSBH__REDIS_URL` to a valid Redis connection string |
