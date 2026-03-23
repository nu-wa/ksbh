+++
title = "Configuration Reference"
description = "Complete configuration reference for KSBH reverse proxy"
weight = 20
+++

# Configuration Reference

This page provides a complete reference for all configuration options available in KSBH.

## Main Configuration Structure

The main configuration is defined in `ksbh_core::config::Config` and consists of the following sections:

```rust
pub struct Config {
    pub cookie_key: Option<String>,    // Cookie signing key (64 bytes min)
    pub redis_url: Option<String>,    // Redis connection URL (optional)
    pub pyroscope_url: Option<String>, // Pyroscope profiling URL
    pub ports: ConfigPorts,           // Port mappings
    pub listen_addresses: ConfigListenAddresses, // Network binding
    pub config_paths: ConfigFilePaths, // File paths
    pub url_paths: ConfigURLPaths,    // URL path prefixes
    pub threads: usize,              // Worker threads (default: 8)
    pub performance: ConfigPerformance, // Performance tuning
    pub constants: ConfigConstants,   // Tunable constants
}
```

---

## Port Configuration

### `ConfigPorts`

Maps internal application ports to external exposed ports:

```yaml
ports:
  app:
    http: 8080    # Internal HTTP port
    https: 8081   # Internal HTTPS port
  external:
    http: 80      # External/public HTTP port
    https: 443    # External/public HTTPS port
```

**Default Values:**

| Field | Default |
|-------|---------|
| `app.http` | `8080` |
| `app.https` | `8081` |
| `external.http` | `80` |
| `external.https` | `443` |

---

## Listen Addresses

### `ConfigListenAddresses`

Configures network interfaces and ports for various services:

```yaml
listen_addresses:
  http: "0.0.0.0:8080"      # Main HTTP listener
  https: "0.0.0.0:8081"     # Main HTTPS listener
  internal: "0.0.0.0:8082"  # Internal admin interface
  profiling: "0.0.0.0:8083" # pprof/profiling endpoint
  prometheus: "0.0.0.0:8084" # Prometheus metrics
```

**Default Values:**

| Address | Default | Purpose |
|---------|---------|---------|
| `http` | `0.0.0.0:8080` | Primary HTTP proxy |
| `https` | `0.0.0.0:8081` | Primary HTTPS proxy |
| `internal` | `0.0.0.0:8082` | Internal health checks, error pages |
| `profiling` | `0.0.0.0:8083` | Performance profiling endpoints |
| `prometheus` | `0.0.0.0:8084` | Prometheus metrics export |

---

## File Paths

### `ConfigFilePaths`

```yaml
config_paths:
  config: "/app/config/config.yaml"      # Typed config path field
  modules: "/app/modules"          # Dynamic module libraries
  static_content: "/app/data/static" # Static files directory
```

**Default Values:**

| Path | Default | Purpose |
|------|---------|---------|
| `config` | `/app/config/config.yaml` | Typed config path field used inside the loaded config |
| `modules` | `/app/modules` | FFI plugin dynamic libraries |
| `static_content` | `/app/data/static` | Static content serving |

---

## URL Paths

### `ConfigURLPaths`

```yaml
url_paths:
  modules: "/_ksbh_internal/"  # Module internal endpoints
```

**Default Values:**

| Path | Default | Purpose |
|------|---------|---------|
| `modules` | `/_ksbh_internal/` | Internal module API endpoints |

---

## Performance Tuning

### `ConfigPerformance`

```yaml
performance:
  tcp_fastopen: 12    # TCP Fast Open queue size
  so_reuseport: false # SO_REUSEPORT socket option
  tcp_keepalive: true # TCP keep-alive probes
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `tcp_fastopen` | `Option<usize>` | `12` | TFO queue size override. Set to `0` to disable. When `null`, uses `constants.tcp_fastopen_queue_size` |
| `so_reuseport` | `Option<bool>` | `null` | Enable SO_REUSEPORT for load balancing across threads |
| `tcp_keepalive` | `Option<bool>` | `null` | Enable TCP keep-alive probes |

### Performance Tuning Notes

- **TFO**: the current default is `12`. If set to `null`, the runtime falls back to `constants.tcp_fastopen_queue_size`.
- **SO_REUSEPORT**: Kernel-level load distribution across worker threads.
- **TCP Keep-Alive**: Detects dead connections through proxies.

---

## Redis Configuration

### `redis_url` (Optional)

Redis is optional. Without it, Redis-dependent features (rate limiting, session storage, OIDC) are disabled.

```bash
KSBH__REDIS_URL="redis://localhost:6379"
# With auth: redis://:password@redis.example.com:6379/0
# Sentinel: redis+sentinel://localhost:26379/mymaster
```

---

## Cookie Security

### `cookie_key` (Required)

Required. Must be at least 64 bytes (base64-encoded).

```bash
KSBH__COOKIE_KEY=$(openssl rand -base64 64)
```

---

## Tunable Constants

### `ConfigConstants`

Internal constants for protocol-level behavior:

```yaml
constants:
  tcp_fastopen_queue_size: 12
  cookie_name: "ksbh"
  cookie_secure: true
  proxy_header_name: "Server"
  proxy_header_value: "ksbh"
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `tcp_fastopen_queue_size` | `usize` | `12` | Default TFO queue size when listener override not set |
| `cookie_name` | `String` | `ksbh` | Name of the session cookie |
| `cookie_secure` | `bool` | `true` | Whether the session cookie is marked `Secure` |
| `proxy_header_name` | `String` | `Server` | HTTP header for server identification |
| `proxy_header_value` | `String` | `ksbh` | Value set in the proxy header |

---

## Thread Configuration

### `threads`

Number of worker threads for handling requests. Default: **8**.

```yaml
threads: 8  # Adjust based on CPU cores
```

---

## Example Complete Configuration

```yaml
# Main settings
cookie_key: "your-64-byte-minimum-key-here"
redis_url: "redis://localhost:6379"
pyroscope_url: "http://pyroscope:4040"
threads: 8

# Port mappings
ports:
  app:
    http: 8080
    https: 8081
  external:
    http: 80
    https: 443

# Network bindings
listen_addresses:
  http: "0.0.0.0:8080"
  https: "0.0.0.0:8081"
  internal: "0.0.0.0:8082"
  profiling: "0.0.0.0:8083"
  prometheus: "0.0.0.0:8084"

# File paths
config_paths:
  config: "/app/config/config.yaml"
  modules: "/app/modules"
  static_content: "/app/data/static"

# URL paths
url_paths:
  modules: "/_ksbh_internal/"

# Performance
performance:
  tcp_fastopen: 12
  so_reuseport: false
  tcp_keepalive: true

# Tunable constants
constants:
  tcp_fastopen_queue_size: 12
  cookie_name: "ksbh"
  cookie_secure: true
  proxy_header_name: "Server"
  proxy_header_value: "ksbh"
```

---

## Environment Variable Mapping

All configuration options can be set via environment variables using double underscore (`__`) as separator and `KSBH` prefix:

| Config Field | Environment Variable |
|--------------|---------------------|
| `cookie_key` | `KSBH__COOKIE_KEY` |
| `redis_url` | `KSBH__REDIS_URL` |
| `pyroscope_url` | `KSBH__PYROSCOPE_URL` |
| `threads` | `KSBH__THREADS` |
| `ports.app.http` | `KSBH__PORTS__APP__HTTP` |
| `ports.app.https` | `KSBH__PORTS__APP__HTTPS` |
| `ports.external.http` | `KSBH__PORTS__EXTERNAL__HTTP` |
| `ports.external.https` | `KSBH__PORTS__EXTERNAL__HTTPS` |
| `listen_addresses.http` | `KSBH__LISTEN_ADDRESSES__HTTP` |
| `listen_addresses.https` | `KSBH__LISTEN_ADDRESSES__HTTPS` |
| `listen_addresses.internal` | `KSBH__LISTEN_ADDRESSES__INTERNAL` |
| `listen_addresses.profiling` | `KSBH__LISTEN_ADDRESSES__PROFILING` |
| `listen_addresses.prometheus` | `KSBH__LISTEN_ADDRESSES__PROMETHEUS` |
| `config_paths.config` | `KSBH__CONFIG_PATHS__CONFIG` |
| `config_paths.modules` | `KSBH__CONFIG_PATHS__MODULES` |
| `config_paths.static_content` | `KSBH__CONFIG_PATHS__STATIC_CONTENT` |
| `url_paths.modules` | `KSBH__URL_PATHS__MODULES` |
| `performance.tcp_fastopen` | `KSBH__PERFORMANCE__TCP_FASTOPEN` |
| `performance.so_reuseport` | `KSBH__PERFORMANCE__SO_REUSEPORT` |
| `performance.tcp_keepalive` | `KSBH__PERFORMANCE__TCP_KEEPALIVE` |
| `constants.tcp_fastopen_queue_size` | `KSBH__CONSTANTS__TCP_FASTOPEN_QUEUE_SIZE` |
| `constants.cookie_name` | `KSBH__CONSTANTS__COOKIE_NAME` |
| `constants.cookie_secure` | `KSBH__CONSTANTS__COOKIE_SECURE` |
| `constants.proxy_header_name` | `KSBH__CONSTANTS__PROXY_HEADER_NAME` |
| `constants.proxy_header_value` | `KSBH__CONSTANTS__PROXY_HEADER_VALUE` |

See [Environment Variables](/docs/configuration/env-variables/) for complete list including security-sensitive options.

Note that the runtime config file lookup path is a separate concern from `config_paths.config`: the process-level file provider path is typically supplied through `KSBH__CONFIG_PATHS__CONFIG`, and the chart defaults that to `/app/config/config.yaml`.
