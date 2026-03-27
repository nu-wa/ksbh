+++
title = "Kubernetes CRDs"
weight = 33
path = "/deployment/crds/"
+++

# Kubernetes CRDs

## ModuleConfiguration CRD

The chart installs the CRD from `charts/ksbh/crds/`, or you can apply it manually:

```bash
kubectl apply -f charts/ksbh/crds/
```

## ModuleConfiguration Spec

| Field | Type | Description |
|-------|------|-------------|
| `spec.name` | string | Module name |
| `spec.type` | string | Module type |
| `spec.weight` | integer | Higher values run earlier within the same scope |
| `spec.global` | boolean | Whether the module is global |
| `spec.requiresBody` | boolean | Whether the module needs the request body |
| `spec.secretRef` | object | Secret reference for module config |

## Built-In Types

Prefer the canonical names:

- `HttpToHttps`
- `OIDC`
- `POW`
- `RateLimit`
- `RobotsDotTXT`
- `Custom`

## Example

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: rate-limiter
spec:
  name: rate-limiter
  type: RateLimit
  weight: 100
  global: true
```

## Ingress Integration

KSBH watches `Ingress` resources with class `ksbh`:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: my-app
spec:
  ingressClassName: ksbh
  rules:
    - host: example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: my-service
                port:
                  number: 80
```

Per-module configuration lives in `ModuleConfiguration` resources. Keep CRD examples aligned with the actual chart and controller behavior.
