---
title: Proof of Work Module
description: Learn how to configure the PoW challenge module for bot mitigation
---

# Proof of Work Module

The Proof of Work (PoW) module provides bot mitigation by presenting computational challenges to unknown clients. Clients must solve a cryptographic puzzle before being allowed to access the protected resource.

### Challenge Flow

1. Check for stored completion state in the session backend
2. If the request is a normal GET and the client is not marked complete → return a challenge HTML page
3. Client solves hash puzzle, POSTs solution to `/{internal_path}/pow`
4. Valid solution → persist completion state for 24 hours, reduce score, redirect to target
5. Invalid solution → return 400
6. Verification challenges themselves expire after a short time window

## Configuration Options

The PoW module supports the following configuration options:

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `difficulty` | string | No | `"4"` | Base number of leading zeros required in hash |
| `secret` | string | No | (built-in) | Server secret for challenge generation |
| `cookie_domain` | string | No | (request host) | Host/domain used when building the PoW action URL |
| `requires_body` | bool | Yes for verification | `false` | Must be `true` if the module needs to read the POST body during challenge verification |

### difficulty

The `difficulty` setting controls how hard the proof-of-work challenge is:

- **Type**: String (parsed as integer)
- **Default**: `"4"`
- **Minimum**: 1 (values below 1 are clamped to 1)
- **Higher values** = more computation required

The difficulty represents the number of leading zeros required in the SHA-256 hash of the solution.

### secret

The `secret` is used to generate unique challenges for each client:

- **Type**: String
- **Default**: A built-in default (change in production!)
- **Minimum length**: 32 bytes

The secret is used as a keyed hash (BLAKE3) input to generate challenges, making them unique per deployment.

### cookie_domain

The `cookie_domain` setting controls the host/domain embedded in the challenge action URL:

- **Type**: String
- **Default**: The request host
- **Use case**: Pointing the challenge form at a shared host

## Challenge Path

The module exposes an internal endpoint for challenge verification:

```
{internal_path}/pow
```

For example, if `internal_path` is `/_ksbh_internal` (the default), the full path is `/_ksbh_internal/pow`.

This path is used internally for POST requests containing the challenge solution. Clients don't directly access this path - it's handled automatically by the module's JavaScript challenge page.

> **Note**: The completion marker is stored in the session backend for 24 hours.

## Request Body Requirement

PoW verification submits a form POST containing:

- `challenge`
- `nonce`

That means the module must receive the request body on the verification endpoint.

For file-based module definitions, set:

```yaml
requires_body: true
```

If `requires_body` is left as `false`, the verification POST can reach the module without the form body being loaded, which causes errors such as `Invalid Form Data`.

## Difficulty Scaling

Difficulty scales with client score: `actual_difficulty = configured_difficulty + (score / 100)`. Clients with higher scores face harder challenges, and the issued effective difficulty is signed into the challenge and enforced during verification.

## Challenge Expiration

Issued challenges are time-bounded during verification. Completion state is then persisted for 24 hours.

## Module Response

### Challenge Page (401)

When presenting a challenge, the module returns:

```http
HTTP/1.1 401 Unauthorized
Content-Type: text/html; charset=utf-8
Content-Length: <html_size>
```

With an HTML page containing:
- Challenge string
- Difficulty level
- JavaScript to compute the solution
- Form submission to the challenge endpoint

### Challenge Payload

The challenge payload has three dot-separated parts:

```text
<issued_at>.<effective_difficulty>.<signature>
```

The signature is derived from:
- the client metrics key
- `issued_at`
- `effective_difficulty`

This lets verification enforce the exact difficulty that was issued on the challenge page.

### Verification Errors

| Error | Status | Description |
|-------|--------|-------------|
| Invalid signature | 400 | Challenge string was tampered with |
| Invalid difficulty | 400 | Issued challenge difficulty was malformed |
| Challenge expired | 400 | Challenge is older than 5 minutes |
| Invalid proof | 400 | Hash doesn't meet difficulty |
| Invalid method | 400 | Not a POST request |

## Example YAML Configuration

### File-Based Configuration

```yaml
modules:
  - name: pow-protection
    type: POW
    weight: 80
    requires_body: true
    global: false
    config:
      difficulty: "4"
      secret: "your-32-byte-secret-key-here!"
      cookie_domain: ".example.com"

ingresses:
  - name: protected-site
    host: www.example.com
    modules:
      - pow-protection
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

### Kubernetes Configuration

First, create a `ModuleConfiguration` resource:

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: pow-protection
spec:
  name: pow-protection
  type: POW
  weight: 80
  global: false
  secretRef:
    name: pow-secret
```

Create a secret with the secret value:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: pow-secret
type: Opaque
stringData:
  secret: "your-32-byte-secret-key-here!"
```

Then reference it in your ingress:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: protected-site
  annotations:
    modules.ksbh.rs/modules: "pow-protection"
spec:
  ingressClassName: ksbh
  rules:
    - host: www.example.com
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

- **Type Code**: `POW` (case-insensitive variants: `pow`, `proofofwork`, `proof-of-work`, `proof of work`)
- **Weight**: explicit per module instance
- **Requires Proper Request**: Yes
- **Requires Body**: Set `requires_body: true` when using this module through file-based configuration so verification POST bodies are available

## Local Demo

Use the repo task to try the module locally:

```bash
mise run run-pow
```

That demo:

- serves the challenge on `http://local.pow.test.local:18080/`
- uses the file provider
- generates a working PoW config automatically
- depends on `requires_body: true` for verification POSTs

You can override the default difficulty:

```bash
KSBH_POW_DIFFICULTY=10 mise run run-pow
```



## Common Use Cases

### Protecting Public Endpoints

```yaml
modules:
  - name: public-endpoint-pow
    type: POW
    weight: 80
    global: false
    config:
      difficulty: "3"
      secret: "change-this-to-a-secure-32-byte-secret"

ingresses:
  - name: public-api
    host: api.example.com
    modules:
      - public-endpoint-pow
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: public-api
          port: 8080
```

### Combined with Rate Limiting

```yaml
modules:
  - name: rate-limit
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
      secret: "your-32-byte-secret-key-here!"

ingresses:
  - name: protected-app
    host: app.example.com
    modules:
      - rate-limit
      - pow-challenge
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: app-backend
          port: 80
```

## Notes

- ordering is controlled by explicit `weight`
- Only challenges GET requests; other methods pass through
- Default secret is for development only - set your own in production
