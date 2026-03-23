+++
title = "Production Considerations"
weight = 34
path = "/deployment/production/"
+++

# Production Considerations

## Redis

KSBH can start without Redis, but shared-state features become limited without it. In practice, `rate-limit`, `oidc`, and `proof-of-work` are the first places where missing Redis hurts.

Example connection string:

```text
redis://redis.example.com:6379/0
```

## TLS

The repo-local Kubernetes path is Helm plus Kubernetes TLS Secrets. Keep TLS operational guidance aligned with what the runtime and chart actually expose instead of assuming extra environment variables.

## Dynamic Modules

Dynamic modules are loaded from `KSBH__CONFIG_PATHS__MODULES` and must be present as shared libraries (`.so` on Linux).

```bash
export KSBH__CONFIG_PATHS__MODULES=/app/modules
```

## Logging

Use `DEBUG_LEVEL` to tune verbosity:

```bash
DEBUG_LEVEL=pingora=info,ksbh_core=debug,ksbh=debug
```

## Monitoring

The chart's Service already includes Prometheus scrape annotations. The default metrics endpoint is:

```text
http://<service>:8084/metrics
```

The internal health endpoint remains on the internal listener and is not exposed by the chart's Service by default:

```text
http://<pod>:8082/healthz
```

## Performance

Thread count is controlled with:

```bash
KSBH__THREADS=16
```

Tune that alongside your CPU limits and real traffic profile rather than copying generic numbers blindly.
