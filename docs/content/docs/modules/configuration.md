---
title: Module Configuration
description: Configure module definitions, ordering, exclusions, and ingress attachment
---

# Module Configuration

This page covers how modules are defined, ordered, attached to ingresses, and excluded when needed.

## At A Glance

- every module definition needs an explicit `weight`
- higher `weight` runs earlier
- global modules always run before ingress modules
- file provider exclusions use `excluded_modules`
- Kubernetes exclusions use `ksbh.rs/excluded-modules`

## File-Based Module Definitions

Each module definition lives under `modules:`:

```yaml
modules:
  - name: <unique-name>
    type: <module-type>
    weight: <i32>
    global: <true|false>
    requires_body: <true|false>
    config:
      <key>: <value>
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Unique module instance name |
| `type` | string | Yes | Module type |
| `weight` | integer | Yes | Higher values run earlier within the same scope |
| `global` | boolean | No | Apply to all ingresses when `true` |
| `requires_body` | boolean | No | Load the request body before module execution |
| `config` | object | No | Module-specific key/value configuration |

## Module Type Aliases

File-based type parsing is case-insensitive for built-in modules.

| Canonical Type | Accepted File Aliases |
|----------------|-----------------------|
| `RateLimit` | `ratelimit`, `rate_limit`, `rate-limit`, `rate limit` |
| `HttpToHttps` | `httpstohttps`, `http_to_https`, `http-to-https`, `http2https`, `http to https` |
| `RobotsDotTXT` | `robotstxt`, `robots_txt`, `robots.txt`, `robotsdottxt` |
| `OIDC` | `oidc` |
| `POW` | `pow`, `proofofwork`, `proof-of-work`, `proof of work` |

## Global vs Ingress Modules

### Global Modules

Global modules apply to every ingress:

```yaml
modules:
  - name: enforce-https
    type: http-to-https
    weight: 100
    global: true
```

### Ingress Modules

Ingress modules are declared globally but attached only where needed:

```yaml
modules:
  - name: app-auth
    type: oidc
    weight: 50
    global: false
    config:
      issuer_url: "https://auth.example.com"
      client_id: "my-app"
      client_secret: "$OIDC_CLIENT_SECRET"

ingresses:
  - name: protected-app
    host: app.example.com
    modules:
      - app-auth
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: my-service
          port: 80
```

## Ordering Rules

Execution is a two-phase pipeline:

1. sort global modules by `weight` descending
2. sort ingress modules by `weight` descending
3. run globals first, then ingress modules

If two modules have the same `weight`, the module name is used as a deterministic tie-break.

### Example

```yaml
modules:
  - name: redirect-http
    type: http-to-https
    weight: 100
    global: true

  - name: oidc-main
    type: oidc
    weight: 1000
    global: false
```

If `oidc-main` is attached to an ingress, `redirect-http` still runs first because global modules are always evaluated before ingress modules.

## Excluding Global Modules

### File Provider

Use `excluded_modules` on the ingress:

```yaml
modules:
  - name: redirect-http
    type: http-to-https
    weight: 100
    global: true

  - name: app-auth
    type: oidc
    weight: 50
    global: false
    config:
      issuer_url: "https://auth.example.com"
      client_id: "my-app"
      client_secret: "$OIDC_CLIENT_SECRET"

ingresses:
  - name: app
    host: app.example.com
    modules:
      - app-auth
    excluded_modules:
      - redirect-http
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: app
          port: 80
```

### Kubernetes Provider

Use the ingress annotation:

```yaml
metadata:
  annotations:
    ksbh.rs/modules: "app-auth"
    ksbh.rs/excluded-modules: "redirect-http"
```

## Ingress Configuration

Each ingress can reference multiple module names:

```yaml
ingresses:
  - name: <ingress-name>
    host: <hostname>
    modules:
      - <module-name-1>
      - <module-name-2>
    excluded_modules:
      - <global-module-name>
    paths:
      - path: <path>
        type: <exact|prefix|implementation-specific>
        backend: <service|static|self>
        service:
          name: <service-name>
          port: <port>
```

## Environment Variable Resolution

Values can reference environment variables using `$VAR_NAME`:

```yaml
config:
  client_secret: "$OIDC_CLIENT_SECRET"
```

If `<VAR>_FILE` is set in the environment, the file provider prefers the file contents over the plain environment value.

## Kubernetes ModuleConfiguration

Modules are represented as `ModuleConfiguration` resources:

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

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `spec.name` | string | Module name |
| `spec.type` | string | Canonical module type |
| `spec.weight` | integer | Higher values run earlier within the same scope |
| `spec.global` | boolean | Apply to all ingresses |
| `spec.config` | object<string,string> | Inline module configuration key/value pairs |
| `spec.secretRef` | object | Kubernetes Secret reference for module config |
| `spec.requiresProperRequest` | boolean | Whether the module expects a proper request |
| `spec.requiresBody` | boolean | Whether the module needs the request body |

If both `spec.config` and `spec.secretRef` are set, secret values override inline keys on conflict.

### Referencing Modules From Ingress

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: my-app
  annotations:
    ksbh.rs/modules: "oidc-auth,rate-limit"
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
                name: my-app
                port:
                  number: 80
```

## Troubleshooting

### Module Not Running

1. Check the module name is correctly attached to the ingress.
2. Ensure `weight` is present on the module definition.
3. For file-based exclusions, verify `excluded_modules` is not removing the module.
4. Check KSBH logs for module load errors.

### Ordering Looks Wrong

1. Confirm the module `weight` values.
2. Remember that global modules always run before ingress modules.
3. If weights match, remember name ordering is the tie-break.

### Body-Dependent Modules Failing

If a module verifies form or JSON POST bodies, set `requires_body: true`. PoW verification is the canonical example.

## See Also

- [Modules Overview](/docs/modules/)
- [YAML Configuration](../configuration/yaml-config/)
- [Configuration Providers](../configuration/providers/)
