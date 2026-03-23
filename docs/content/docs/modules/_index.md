+++
title = "Modules"
description = "Overview of the KSBH module system"
weight = 40
+++

# Modules Overview

Modules are request filters that run inside the proxy pipeline. They can inspect a request, mutate proxy state, or stop the request early by writing a response.

## What Modules Are For

Use modules when you want KSBH to do work before the request reaches the backend, for example:

- rate-limit abusive clients
- present a proof-of-work challenge
- enforce OIDC authentication
- serve `robots.txt`
- redirect HTTP traffic to HTTPS

## How Modules Attach

Modules are declared once, then attached in one of two scopes:

- **global modules**: `global: true`, applied to every ingress
- **ingress modules**: attached only to a specific ingress

Global modules always run before ingress modules.

## Ordering Model

Ordering is driven by explicit instance `weight`, not by module type.

- higher `weight` runs earlier
- global modules are sorted only against other global modules
- ingress modules are sorted only against other ingress modules
- final execution order is: sorted globals first, then sorted ingress modules
- equal weights are resolved deterministically by module name

That means a high-weight ingress module does **not** jump ahead of a lower-weight global module. Scope comes first, then weight within that scope.

### Example

```yaml
modules:
  - name: http-redirect
    type: http-to-https
    weight: 100
    global: true

  - name: oidc-main
    type: oidc
    weight: 1000
    global: false
```

If `oidc-main` is attached to an ingress, execution is still:

1. `http-redirect`
2. `oidc-main`

because the global phase runs before the ingress phase.

## Excluding Global Modules

An ingress can exclude selected global modules.

- file provider: use `excluded_modules`
- Kubernetes provider: use the `ksbh.rs/excluded-modules` annotation

This is useful when a module should be global by default but skipped for a particular ingress.

## File and Kubernetes Type Names

Built-in modules use the same underlying enum in both providers.

| Module | Purpose | Canonical Kubernetes Type | File Provider |
|--------|---------|---------------------------|---------------|
| [Rate Limit](./rate-limit) | Limits request rate based on client score | `RateLimit` | case-insensitive aliases accepted |
| [Proof of Work](./proof-of-work) | Bot mitigation via computational challenges | `POW` | case-insensitive aliases accepted |
| [OIDC](./oidc) | OpenID Connect authentication | `OIDC` | case-insensitive aliases accepted |
| [Robots.txt](./robots-txt) | Serves custom robots.txt content | `RobotsDotTXT` | case-insensitive aliases accepted |
| [HTTP to HTTPS](./http-to-https) | Redirects HTTP to HTTPS | `HttpToHttps` | case-insensitive aliases accepted |

## Minimal Example

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
      client_id: "ksbh-proxy"
      client_secret: "$OIDC_CLIENT_SECRET"

ingresses:
  - name: my-app
    host: app.example.com
    modules:
      - app-auth
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: my-app
          port: 80
```

## Start Here

- [Module Configuration](./configuration) for ordering, scoping, exclusions, and YAML/Kubernetes examples
- [Rate Limit](./rate-limit) for score-based request throttling
- [Proof of Work](./proof-of-work) for bot mitigation
- [OIDC](./oidc) for authenticated applications
- [Robots.txt](./robots-txt) for crawler control
- [HTTP to HTTPS](./http-to-https) for redirect handling
