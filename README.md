# KSBH

A modular reverse proxy server built on top of [pingora](https://github.com/cloudflare/pingora).

> [!NOTE]  
> **AI Disclaimer**: AI was used in the project.

## Why KSBH?

I run my own Kubernetes cluster, I was using Kong Community as my Ingress Controller, and I was relying on an open-source OIDC lua plugin.
One day it broke, and I just thought why not build my own solution, instead of trying to fix the already *working* Ingress Controller I had in place, as you would.

I thought of the project as a simple reverse proxy at first, and decided to stick with what I knew, running an Ingress Controller in Kubernetes, but always had modularity in mind.

## Features

- **HTTP/HTTPS Reverse Proxy**: Built on pingora for high performance
- **Dynamic TLS**: SNI-based certificate lookup with automatic certificate rotation
- **Modular request filtering Architecture**: Module system via FFI dynamic libraries
- **Kubernetes Integration**: Configuration via Custom Resources or file-based config
- **Built-in Modules**:
  - HTTP to HTTPS redirection
  - OIDC authentication
  - Proof-of-work challenge (bot protection)
  - Rate limiting
  - robots.txt handling

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         ksbh                                │
│                    (cargo package: ksbh)                    │
├─────────────────────────────────────────────────────────────┤
│  ksbh-bin (entry point)                                     │
│  ├── Static content (health checks, error pages)            │
│  ├── TLS termination                                        │
│  └── Orchestrates ksbh-core + config provider + modules     │
├─────────────────────────────────────────────────────────────┤
│  ksbh-core                                                  │
│  ├── Proxy logic & request handling                         │
│  ├── Routing (host + path based)                            │
│  ├── Module system (FFI plugins)                            │
│  ├── Storage (Redis)                                        │
│  ├── Config providers                                       │
│  └── generate_crd (dev utility binary)                      │
├─────────────────────────────────────────────────────────────┤
│  ksbh-modules (cdylib, loaded dynamically at runtime)       │
│  ├── http_to_https                                          │
│  ├── oidc                                                   │
│  ├── proof-of-work                                          │
│  ├── rate-limit                                             │
│  └── robots-txt                                             │
├─────────────────────────────────────────────────────────────┤
│  ksbh-modules-sdk (FFI bindings for custom modules)         │
├─────────────────────────────────────────────────────────────┤
│  ksbh-types (shared type definitions)                       │
├─────────────────────────────────────────────────────────────┤
│  Config Providers:                                          │
│  ├── ksbh-config-providers-file (YAML file watching)        │
│  └── ksbh-config-providers-kubernetes (K8s CRD + Ingress)   │
└─────────────────────────────────────────────────────────────┘
```

## Modules

KSBH uses an FFI-based module system. Modules are compiled as `cdylib` (C-compatible dynamic libraries) and loaded at runtime via `libloading`.

### Built-in Modules

| Module | Description |
|--------|-------------|
| `http_to_https` | Redirects HTTP requests to HTTPS |
| `oidc` | OpenID Connect authentication with PKCE, token refresh, and OIDC discovery |
| `proof-of-work` | Hashcash-style PoW challenge for bot protection with dynamic difficulty scaling and caching |
| `rate-limit` | Metrics-based rate limiting using score thresholds |
| `robots-txt` | Serves static robots.txt content |

## Alternatives

If you're looking for a production-grade solution, consider:

- [Kong](https://konghq.com/)
- [Caddy](https://caddyserver.com/)
- [Traefik](https://traefik.io/)
- [Envoy](https://www.envoyproxy.io/)

## License

MIT
