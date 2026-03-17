# ksbh-bin

Main binary entry point for the KSBH reverse proxy server.

## Purpose

This crate is the executable entry point that:
- Starts the pingora-based HTTP/HTTPS server
- Integrates all modules (http_to_https, oidc, rate-limit, etc.)
- Handles TLS configuration
- Manages static content serving
- Provides Kubernetes integration

## Module Structure

The crate is organized into the following internal modules:

- `apps/` - Static content serving with file caching & compression
- `proxy/` - Pingora proxy wrapper (PingoraSessionWrapper, PingoraWrapper)
- `services/` - Background services (module loading, metrics)
- `tls/` - Dynamic TLS with SNI support

## Runtime Components

### JWT Keys

The following global JWT keys are initialized in `main.rs`:
- `JWT_ENC_ENC_KEY`: Encoding key for JWT token generation (via `ENV_JWT_PEM_ENCODE`)
- `JWT_ENC_DEC_KEY`: Decoding key for JWT token validation (via `ENV_JWT_PEM_DECODE`)

### Config Providers

Configuration is loaded via pluggable providers determined by `KSBH_CONFIG_PATH` env var:
- **File-based**: Set `KSBH_CONFIG_PATH` to a file path to use `ksbh-config-providers-file`
- **Kubernetes-based**: If `KSBH_CONFIG_PATH` is not set, uses `ksbh-config-providers-kubernetes`

### Redis Storage

Session storage is created in `server.rs` using `RedisHashMap`:
- **Hot (in-memory) TTL**: 24 hours
- **Cold (Redis) TTL**: 48 hours

## Features

- `profiling` (default): Enables Pyroscope + jemalloc pprof profiling
  - Uses `tikv-jemallocator` with profiling enabled
  - Provides pprof data via Pyroscope agent

- Metrics: Prometheus endpoint for monitoring (see Services section)

## Services

The following pingora services are started (in order):

1. `static_internal` - Internal HTTP service for health checks, error pages, static files
2. `config_service` - Configuration loading and watching
3. `background_service` - Module loading and management
4. `metrics_service` - Metrics collection and storage
5. `prom_service` - Prometheus metrics endpoint
6. `proxy_service` - Main reverse proxy service
7. `profiling_service` (optional, when `profiling` feature enabled) - pprof endpoint

## Static Content

The static content service (`apps/static_content/mod.rs`) provides:

- **File Caching**: Uses `FileCache` (memory-mapped files) for efficient static file serving
- **Compression**: Supports multiple compression algorithms (prioritized by client preference):
  - Brotli (`br`)
  - Gzip (`gzip`)
  - Deflate (`deflate`)
  - Zlib (`zlib`)
- **Templates**: Error page templates (400, 401, 403, 404, 500, 502) rendered via Askama
- **Endpoints**:
  - `/healthz` - Health check
  - `/static?file=<filename>` - Serve static files
  - `/400`, `/401`, `/403`, `/404`, `/500`, `/502` - Error pages

## Dynamic TLS

The TLS implementation (`tls/mod.rs`) supports:

- **SNI (Server Name Indication)**: Extracts SNI from client hello to determine certificate
- **Certificate Lookup**: Uses `CertsRegistry` to fetch certificates by domain name
- **Default Certificate**: Falls back to generated ED25519 self-signed certificate if no match
- **Cipher Suites**: Restricted to `TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256`
- **HTTP/2**: Enabled via `tls_settings.enable_h2()`

## Key Dependencies

- `ksbh-core`: Core library functionality
- `pingora`: HTTP server framework
- `kube`: Kubernetes client
- All module crates: http_to_https, oidc, proof-of-work, rate-limit, robots-txt

## Build

```bash
cargo build -p ksbh
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
