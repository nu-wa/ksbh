---
title: HTTP to HTTPS Redirect Module
description: Learn how to configure automatic HTTP to HTTPS redirects
---

# HTTP to HTTPS Redirect Module

The HTTP to HTTPS Redirect module automatically redirects incoming HTTP requests to their HTTPS equivalents. This ensures all traffic to your services is encrypted.

### Request Flow

```aasvg
Incoming Request
      │
      ▼
┌─────────────────────┐
│ Is HTTPS Request?   │──Yes──> Pass Through
│ (scheme == https   │
│  OR port == 443    │
│  OR uri starts     │
│  with https://)   │
└─────────────────────┘
      │ No
      ▼
┌─────────────────────┐
│ Convert URL to HTTPS│
│ (http:// -> https://)
└─────────────────────┘
      │
      ▼
┌─────────────────────┐
│ Return 301 Redirect│
│ Location: https:// │
└─────────────────────┘
```

## How It Detects Insecure Requests

The module redirects only true HTTP requests. It treats the following as already secure and lets them pass through:

| Condition | Description |
|-----------|-------------|
| Scheme is `https` | HTTPS request |
| Port is `443` | HTTPS default port |
| URI starts with `https://` | Absolute HTTPS URI |
| URI starts with `wss://` | Secure WebSocket URI |

## Configuration Options

This module does **not require any configuration**:

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| (none) | - | - | - | No configuration needed |

The module works out of the box with zero configuration.

## Redirect Response

When redirecting, the module returns:

```http
HTTP/1.1 301 Moved Permanently
Location: https://example.com/path/to/resource
Content-Length: 0
```

## Example YAML Configuration

### File-Based Configuration

The module is typically used as a global module to catch all HTTP traffic:

```yaml
modules:
  - name: http-to-https
    type: HttpToHttps
    weight: 100
    global: true
    # No config needed!

ingresses:
  - name: secure-app
    host: app.example.com
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

### With File-Based TLS Configured

Here's a complete example with TLS and HTTP redirect:

```yaml
modules:
  - name: http-to-https
    type: HttpToHttps
    weight: 100
    global: true

ingresses:
  - name: secure-app
    host: app.example.com
    tls:
      cert_file: /etc/ksbh/certs/app.example.com.crt
      key_file: /etc/ksbh/certs/app.example.com.key
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

### Kubernetes Configuration

Create a global `ModuleConfiguration`:

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: http-to-https
spec:
  name: http-to-https
  type: HttpToHttps
  weight: 100
  global: true
```

Then create an ingress that requires HTTPS (using TLS):

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: secure-app
  annotations:
    modules.ksbh.rs/modules: ""
spec:
  ingressClassName: ksbh
  tls:
    - hosts:
        - app.example.com
      secretName: app-tls-cert
  rules:
    - host: app.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: web-backend
                port:
                  number: 80
```

## Module Properties

- **Type Code**: `HttpToHttps` (PascalCase required for Kubernetes; file-based config accepts case-insensitive variants)
- **Weight**: explicit per module instance
- **Requires Proper Request**: Yes
- **Requires Body**: No

## Common Use Cases

### Enforcing HTTPS for All Traffic

The most common use case is to enforce HTTPS for all services:

```yaml
modules:
  - name: enforce-https
    type: HttpToHttps
    weight: 100
    global: true
```

This should be combined with TLS configuration on your ingresses. In file-based config, that means `cert_file` and `key_file`; in Kubernetes, that means `spec.tls` plus a Secret.

### Specific Host Redirect

To redirect only specific hosts:

```yaml
modules:
  - name: redirect-main-domain
    type: HttpToHttps
    weight: 100
    global: false

ingresses:
  - name: main-site
    host: example.com
    modules:
      - redirect-main-domain
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

This won't affect other hosts like `api.example.com`.

## Notes

- ordering is controlled by explicit `weight`
- 301 is cached permanently by browsers
- Redirects preserve path, query string, and fragment
- `wss://` passes through without redirect
