+++
title = "Kubernetes Deployment"
weight = 32
path = "/deployment/kubernetes/"
+++

# Kubernetes Deployment

## Helm Chart

Chart location: `charts/ksbh/`

## Quick Start

```bash
helm install ksbh ./charts/ksbh --namespace ksbh --create-namespace
```

## Key Chart Values

The values file currently centers on these knobs:

```yaml
configProvider:
  mode: kubernetes

image:
  repository: ksbh
  tag: latest

service:
  type: LoadBalancer
  http: 8080
  https: 8081
  profiling: 8083
  prometheus: 8084

app:
  redisUrl: "redis://redis.ksbh.svc.cluster.local:6379/0"
  threads: 8
  trustedProxies:
    - "10.0.0.0/8"

configEnv:
  KSBH__CONSTANTS__COOKIE_SECURE: "false"

env:
  DEBUG_LEVEL: "info"
```

## Configuration Provider Modes

- `configProvider.mode=file`: mount a YAML config file and set `KSBH__CONFIG_PATHS__CONFIG`
- `configProvider.mode=kubernetes`: let KSBH watch cluster resources

Runtime env vars derived from chart `app.*` values (for example `app.redisUrl`,
`app.pyroscopeUrl`, `app.threads`, `app.constants.*`, `app.ports.*`,
`app.listenAddresses.*`, `app.configPaths.modules`, `app.configPaths.staticContent`,
`app.urlPaths.modules`, `app.performance.*`, `app.trustedProxies`) are injected
in both modes.

`app.configPaths.config` is intentionally file-mode-only because setting
`KSBH__CONFIG_PATHS__CONFIG` selects the file provider.

### File Mode Behavior

When `configProvider.mode=file` is enabled, the chart:

- creates a ConfigMap-backed runtime config file
- mounts it at `app.configPaths.config`
- sets `KSBH__CONFIG_PATHS__CONFIG` in the container
- still mounts application data at `/app/data` by default
- still injects `KSBH__COOKIE_KEY` from a Kubernetes Secret

So file mode is still a Kubernetes deployment. It just selects the file provider inside the pod.

## Service Ports

| Port | Purpose |
|------|---------|
| 8080 | HTTP |
| 8081 | HTTPS |
| 8083 | Profiling |
| 8084 | Prometheus metrics |

The internal health listener remains on `8082`, but it is not exposed by the Kubernetes Service.

## Local kind Workflow

The repo's `mise run e2e-kubernetes-provider` task installs the chart into kind and maps:

- `18080` to HTTP
- `18443` to HTTPS
- `18083` to profiling
- `18084` to Prometheus metrics

## Important Constraint

The chart does not create your application `Ingress` objects for you. Create those separately, and use `configProvider.mode=kubernetes` if you want KSBH to watch cluster `Ingress` and `ModuleConfiguration` resources.
