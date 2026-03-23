---
title: Rate Limit Module
description: Learn how to configure the rate limiting module
---

# Rate Limit Module

The Rate Limit module provides metrics-based request rate limiting using a scoring system. It's designed to protect your services from excessive requests by tracking client behavior and blocking those that exceed a configurable threshold.

### Request Flow

```aasvg
Client Request → [Check Score > threshold?] → No → Pass Through
                                        → Yes → Return 429
```

## Configuration Options

The rate limit module supports the following configuration options:

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `score_threshold` | string | No | `"100"` | The score threshold above which requests are blocked |

### score_threshold

The `score_threshold` configuration option sets the maximum allowed score for a client. When a client's score exceeds this value, the module returns HTTP 429 (Too Many Requests).

- **Type**: String (parsed as integer)
- **Default**: `"100"`
- **Lower values** = more restrictive
- **Higher values** = more permissive

## Scoring

Clients start at score 0. Score increases with negative behavior (blocked IPs, failed PoW, etc.) and decreases when modules call `ctx.metrics.good_boy()` (e.g., after solving PoW). Score > threshold = 429 response.

## Module Response

When a request is blocked, the module returns:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60
X-Score: <current_score>
Content-Length: 0
```

The client can retry after the specified `Retry-After` seconds.

## Example YAML Configuration

### File-Based Configuration

```yaml
modules:
  # Global rate limiting
  - name: global-rate-limit
    type: RateLimit
    weight: 100
    global: true
    config:
      score_threshold: "200"

  # Stricter rate limiting for API
  - name: api-rate-limit
    type: RateLimit
    weight: 50
    global: false
    config:
      score_threshold: "50"

ingresses:
  - name: api-ingress
    host: api.example.com
    modules:
      - api-rate-limit
    paths:
      - path: /api
        type: prefix
        backend: service
        service:
          name: api-service
          port: 8080
```

### Kubernetes Configuration

First, create a `ModuleConfiguration` resource:

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: api-rate-limit
spec:
  name: api-rate-limit
  type: RateLimit
  weight: 50
  global: false
```

Then reference it in your ingress:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: api-ingress
  annotations:
    modules.ksbh.rs/modules: "api-rate-limit"
spec:
  ingressClassName: ksbh
  rules:
    - host: api.example.com
      http:
        paths:
          - path: /api
            pathType: Prefix
            backend:
              service:
                name: api-service
                port:
                  number: 8080
```

## Module Properties

- **Type Code**: `RateLimit` (**Kubernetes**: exact `RateLimit` only; **File provider**: also accepts `ratelimit`, `rate_limit`, `rate-limit`, `rate limit`)
- **Weight**: explicit per module instance
- **Requires Proper Request**: Yes
- **Requires Body**: No

## Common Use Cases

### Protecting Public APIs

```yaml
modules:
  - name: api-protection
    type: RateLimit
    weight: 50
    global: false
    config:
      score_threshold: "100"

ingresses:
  - name: public-api
    host: api.example.com
    modules:
      - api-protection
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: api-backend
          port: 8080
```

### Combined with PoW

The rate limit module works with the PoW (Proof of Work) module. When a client solves a PoW challenge, the PoW module calls `ctx.metrics.good_boy()` to reduce their score by 50, allowing them through the rate limit.

```yaml
modules:
  - name: combined-protection
    type: RateLimit
    weight: 100
    global: false
    config:
      score_threshold: "150"

  - name: pow-challenge
    type: POW
    weight: 80
    global: false
    config:
      difficulty: "4"
      secret: "your-secret-key-at-least-32-bytes"

ingresses:
  - name: protected-app
    host: app.example.com
    modules:
      - combined-protection
      - pow-challenge
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

## Notes

- ordering is controlled by explicit `weight`
- Works with the metrics system; other modules update scores
- `Retry-After` is always 60 seconds; `X-Score` header shows current score for debugging
