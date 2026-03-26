+++
title = "YAML Configuration"
description = "Complete guide to KSBH YAML configuration format"
weight = 40
+++

# YAML Configuration

KSBH supports declarative configuration via YAML files. This is the recommended approach for development and simple deployments.

## Configuration File Location

The YAML configuration file is specified via environment variable:

```bash
export KSBH__CONFIG_PATHS__CONFIG="/app/config/config.yaml"
```

If not specified, the runtime looks for `/app/config/config.yaml`.

That default applies to the top-level runtime config file lookup. It is separate from the typed `config_paths.config` field inside the loaded configuration.

---

## YAML Structure

The configuration file consists of two main sections:

```yaml
# Module definitions (optional)
modules:
  - name: "module-name"
    type: "rate-limit"
    weight: 100
    global: false
    config:
      key: value

# Ingress rules (optional)
ingresses:
  - name: "my-ingress"
    host: "example.com"
    tls:
      cert_file: "/etc/ksbh/certs/example.com.crt"
      key_file: "/etc/ksbh/certs/example.com.key"
    paths:
      - path: "/api"
        type: "prefix"
        backend: "service"
        service:
          name: "api-service"
          port: 8080
    modules:
      - "rate-limit"
```

---

## Module Configuration

### Module Types

KSBH supports the following built-in module types:

| Type | Description |
|------|-------------|
| `rate-limit` | Rate limiting for requests |
| `http-to-https` | Redirect HTTP to HTTPS |
| `robots.txt` | Serve robots.txt file |
| `oidc` | OpenID Connect authentication |
| `pow` | Proof-of-work challenge |
| `custom` | Custom FFI module |

### Module Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Unique module name |
| `type` | string | Yes | Module type (see above) |
| `weight` | integer | Yes | Higher values run earlier within the same scope |
| `requires_body` | boolean | No | Load the request body before module execution (default: false) |
| `global` | boolean | No | Apply to all ingresses (default: false) |
| `config` | object | No | Module-specific configuration |

### Module Configuration Examples

```yaml
modules:
  - name: "rate-limiter"
    type: "ratelimit"
    weight: 100
    global: true
    config:
      score_threshold: "100"

  - name: "https-redirect"
    type: "http-to-https"
    weight: 200
    global: true
    config:
      permanent: "true"

  - name: "robots"
    type: "robots.txt"
    weight: 50
    global: true
    config:
      disallow: "/admin"

  - name: "oidc"
    type: "oidc"
    weight: 50
    config:
      issuer: "$OIDC_ISSUER"
      client_id: "$OIDC_CLIENT_ID"
      client_secret: "$OIDC_CLIENT_SECRET"

  - name: "pow"
    type: "pow"
    weight: 80
    requires_body: true
    config:
      difficulty: "4"
      expires_seconds: "300"
```

For modules that verify form or JSON POST bodies, `requires_body: true` is required. A practical example is the PoW module, whose verification endpoint reads `challenge` and `nonce` from the POST body.

---

## Ingress Configuration

### Ingress Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Unique ingress name |
| `host` | string | Yes | Hostname to match |
| `tls` | object | No | TLS configuration metadata |
| `paths` | array | Yes | List of path rules |
| `modules` | array | No | Modules to apply |

### TLS Configuration

| Field | Type | Description |
|-------|------|-------------|
| `cert_file` | string | Path to certificate file |
| `key_file` | string | Path to private key file |
| `secret_name` | string | Optional metadata label in file mode; Kubernetes Secret name in Kubernetes provider docs |

```yaml
tls:
  cert_file: "/etc/ksbh/certs/example.com.crt"
  key_file: "/etc/ksbh/certs/example.com.key"
```

For the file provider, `cert_file` and `key_file` are the effective TLS fields. `secret_name` is only metadata in YAML mode; Kubernetes Secret-backed TLS is a Kubernetes provider feature.

### Path Configuration

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | Yes | Path to match |
| `type` | string | No | Match type: `exact`, `prefix`. Defaults to `prefix` when omitted. |
| `backend` | string | Yes | Backend type: `service`, `static` |
| `service` | object | For `service` backend | Service details |

### Backend Types

#### Service Backend

Proxies to a Kubernetes service:

```yaml
paths:
  - path: "/api"
    type: "prefix"
    backend: "service"
    service:
      name: "api-service"
      port: 8080
```

#### Static Backend

Serves static content:

```yaml
paths:
  - path: "/static"
    type: "prefix"
    backend: "static"
```

### Path Match Types

| Type | Description | Example |
|------|-------------|---------|
| `exact` | Exact path match | `/api` matches only `/api` |
| `prefix` | Prefix match | `/api` matches `/api`, `/api/users`, etc. |

---

## Complete Examples

### Basic Example

```yaml
# Simple configuration with rate limiting

modules:
  - name: "default-rate-limit"
    type: "ratelimit"
    weight: 100
    global: true
    config:
      score_threshold: "60"

ingresses:
  - name: "main-ingress"
    host: "example.com"
    paths:
      - path: "/"
        type: "prefix"
        backend: "service"
        service:
          name: "webapp"
          port: 80
      - path: "/api"
        type: "prefix"
        backend: "service"
        service:
          name: "api-service"
          port: 8080
    modules:
      - "default-rate-limit"
```

### Full-Featured Example

```yaml
# Complete configuration with file-based TLS, multiple modules

modules:
  # Global: redirect HTTP to HTTPS
  - name: "https-redirect"
    type: "http-to-https"
    weight: 200
    global: true
    config:
      permanent: "true"

  # Global: robots.txt
  - name: "robots"
    type: "robots.txt"
    weight: 40
    global: true
    config:
      disallow: "/admin\n/private"
      allow: "/public"

  # Per-ingress: rate limiting
  - name: "api-limiter"
    type: "ratelimit"
    weight: 100
    global: false
    config:
      score_threshold: "100"

  # Per-ingress: OIDC
  - name: "oidc-auth"
    type: "oidc"
    weight: 50
    global: false
    config:
      issuer: "https://auth.example.com"
      client_id: "ksbh"

ingresses:
  # Public website
  - name: "website"
    host: "example.com"
    tls:
      cert_file: "/etc/ksbh/certs/example.com.crt"
      key_file: "/etc/ksbh/certs/example.com.key"
    paths:
      - path: "/"
        type: "prefix"
        backend: "service"
        service:
          name: "web"
          port: 80
      - path: "/static"
        type: "prefix"
        backend: "static"

  # Authenticated API
  - name: "api"
    host: "api.example.com"
    tls:
      cert_file: "/etc/ksbh/certs/api.example.com.crt"
      key_file: "/etc/ksbh/certs/api.example.com.key"
    paths:
      - path: "/v1"
        type: "prefix"
        backend: "service"
        service:
          name: "api-v1"
          port: 8080
    modules:
      - "oidc-auth"
      - "api-limiter"

  # Health check (no auth)
  - name: "health"
    host: "health.example.com"
    paths:
      - path: "/"
        type: "prefix"
        backend: "service"
        service:
          name: "health-service"
          port: 8080
```

### Environment Variable Interpolation

The YAML configuration supports environment variable interpolation using the `$VAR_NAME` syntax:

```yaml
modules:
  - name: "oidc"
    type: "oidc"
    config:
      issuer: "$OIDC_ISSUER"
      client_id: "$OIDC_CLIENT_ID"
      client_secret: "$OIDC_CLIENT_SECRET"

ingresses:
  - name: "example"
    host: "$HOSTNAME"  # Will use env var
    paths:
      - path: "/"
        backend: "service"
        service:
          name: "$SERVICE_NAME"  # Will use env var
          port: 8080
```

---

## Hot Reload

File changes trigger automatic reload without restart.

---

## Migrating from Kubernetes CRD

If you're migrating from Kubernetes-based configuration to YAML:

| Kubernetes CRD | YAML Equivalent |
|----------------|-----------------|
| `ModuleConfiguration` | `modules[]` |
| `Ingress` (with class `ksbh`) | `ingresses[]` |
| `Ingress.spec.rules` | `ingresses[].paths` |
| `Ingress annotations.ksbh.rs/modules` | `ingresses[].modules` |
| TLS Certificate Files | `ingresses[].tls.cert_file` + `ingresses[].tls.key_file` |
