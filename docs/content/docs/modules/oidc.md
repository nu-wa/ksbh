---
title: OIDC Authentication Module
description: Learn how to configure OpenID Connect authentication
---

# OIDC Authentication Module

The OIDC (OpenID Connect) module provides authentication using OpenID Connect protocol. It implements the standard Authorization Code flow with PKCE (Proof Key for Code Exchange) for secure authentication.

## Purpose and Behavior

The OIDC module:

1. **Redirects unauthenticated users** - Sends them to the OIDC provider for login
2. **Handles callback** - Processes the authorization code returned by the provider
3. **Exchanges tokens** - Exchanges code for tokens (ID token, refresh token)
4. **Manages sessions** - Creates and maintains authenticated sessions
5. **Supports refresh tokens** - Optionally refreshes expired access tokens

### Authentication Flow

Unauthenticated requests redirect to OIDC provider. Provider redirects back with code. Code exchanged for tokens, session cookie set, request proceeds.

### Authorization Code Flow with PKCE

Standard OIDC flow with PKCE for security. PKCE prevents authorization code interception attacks.

## Configuration Options

The OIDC module supports the following configuration options:

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `issuer_url` | string | Yes | - | OIDC provider's issuer URL |
| `client_id` | string | Yes | - | Client ID registered with OIDC provider |
| `client_secret` | string | Yes | - | Client secret for the application |
| `session_ttl_seconds` | string | No | `"3600"` | Session lifetime in seconds |
| `enable_refresh` | string | No | `"false"` | Enable refresh token support |
| `modules_internal_path` | string | No | `"/_ksbh_internal"` | Internal path for callbacks |

### issuer_url

The `issuer_url` is the base URL of your OIDC provider:

- **Type**: String (required)
- **Format**: Full URL (e.g., `https://auth.example.com`)

The module performs OIDC Discovery at this URL to fetch:
- Authorization endpoint
- Token endpoint
- JWKS (JSON Web Key Set) for token verification

### client_id

The `client_id` identifies your application to the OIDC provider:

- **Type**: String (required)
- Register your application with the OIDC provider to get this value

### client_secret

The `client_secret` authenticates your application to the OIDC provider:

- **Type**: String (required)
- **Security**: Store in a Kubernetes Secret or environment variable

### session_ttl_seconds

The `session_ttl_seconds` sets how long the session remains valid:

- **Type**: String (parsed as integer)
- **Default**: `"3600"` (1 hour)
- **Range**: Any positive integer

### enable_refresh

The `enable_refresh` option enables automatic token refresh:

- **Type**: String (`"true"` or `"false"`)
- **Default**: `"false"`

When enabled:
1. When session is about to expire, module attempts refresh
2. Uses the refresh token to get new access token
3. If refresh fails, redirects to login

### modules_internal_path

The `modules_internal_path` sets the callback path prefix:

- **Type**: String
- **Default**: `"/_ksbh_internal"`
- The OIDC callback will be at `/{modules_internal_path}/oidc`

## Callback Path

The module exposes an internal endpoint for OIDC callbacks:

```
/_ksbh_internal/oidc
```

This handles the OAuth2/OIDC callback with:
- `code`: Authorization code from provider
- `state`: CSRF protection token

## Kubernetes Secret Reference

In Kubernetes deployments, sensitive values like `client_secret` should be stored in Secrets:

### Creating the Secret

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: oidc-credentials
type: Opaque
stringData:
  client_id: "ksbh-proxy"
  client_secret: "your-client-secret"
```

### Referencing in ModuleConfiguration

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: oidc-auth
spec:
  name: oidc-auth
  type: OIDC
  weight: 50
  global: false
  secretRef:
    name: oidc-credentials
```

The module will automatically load `client_id` and `client_secret` from the referenced secret.
You can also set keys inline via `spec.config`, but `secretRef` is recommended for credentials.

## Example YAML Configuration

### File-Based Configuration

```yaml
modules:
  - name: oidc-auth
    type: OIDC
    weight: 50
    global: false
    config:
      issuer_url: "https://accounts.google.com"
      client_id: "your-client-id"
      client_secret: "$OIDC_CLIENT_SECRET"
      session_ttl_seconds: "3600"
      enable_refresh: "false"

ingresses:
  - name: protected-app
    host: app.example.com
    modules:
      - oidc-auth
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

### Kubernetes Configuration

First, create a Secret with credentials:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: oidc-credentials
type: Opaque
stringData:
  client_id: "ksbh-proxy"
  client_secret: "your-client-secret-value"
```

Then create the `ModuleConfiguration`:

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: oidc-auth
spec:
  name: oidc-auth
  type: OIDC
  global: false
  secretRef:
    name: oidc-credentials
```

Finally, reference in your ingress:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: protected-app
  annotations:
    modules.ksbh.rs/modules: "oidc-auth"
spec:
  ingressClassName: ksbh
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

### With Token Refresh

```yaml
modules:
  - name: oidc-auth-with-refresh
    type: OIDC
    weight: 50
    global: false
    config:
      issuer_url: "https://auth.example.com"
      client_id: "ksbh-proxy"
      client_secret: "$OIDC_CLIENT_SECRET"
      session_ttl_seconds: "7200"
      enable_refresh: "true"
```

## Module Properties

- **Type Code**: `OIDC` (exact casing required for Kubernetes)
- **Weight**: explicit per module instance
- **Requires Proper Request**: Yes
- **Requires Body**: No
- **Flow State TTL**: 5 minutes (flow must complete within this time)

## Session Management

Sessions stored in Redis. Cookie contains session ID. TTL per `session_ttl_seconds`.

## Common Use Cases

### Protecting Internal Applications

```yaml
modules:
  - name: corporate-oidc
    type: OIDC
    weight: 50
    global: false
    config:
      issuer_url: "https://sso.company.com"
      client_id: "internal-apps"
      client_secret: "$OIDC_SECRET"
      session_ttl_seconds: "28800"  # 8 hours

ingresses:
  - name: internal-tool
    host: tool.internal.company.com
    modules:
      - corporate-oidc
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: internal-tool
          port: 8080
```

### Combined with PoW for Extra Protection

```yaml
modules:
  - name: pow-challenge
    type: POW
    weight: 80
    global: false
    config:
      difficulty: "4"
      secret: "your-32-byte-secret"

  - name: oidc-auth
    type: OIDC
    weight: 50
    global: false
    config:
      issuer_url: "https://auth.example.com"
      client_id: "ksbh-app"
      client_secret: "$OIDC_SECRET"

ingresses:
  - name: secured-app
    host: app.example.com
    modules:
      - pow-challenge
      - oidc-auth
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
- OIDC and PoW sessions are independent
- Uses `openidconnect` crate for token validation
- Always use HTTPS in production
