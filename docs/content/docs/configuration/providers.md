+++
title = "Configuration Providers"
description = "Guide to KSBH configuration providers: File-based and Kubernetes"
weight = 50
+++

# Configuration Providers

Two providers are available:

- **File-based**: YAML files with hot reload
- **Kubernetes**: Custom Resources and Ingress

---

## At A Glance

- set `KSBH__CONFIG_PATHS__CONFIG` to use the file provider
- leave it unset to use the Kubernetes provider
- file provider supports YAML reload and environment interpolation
- file provider supports ingress TLS from `cert_file` and `key_file`
- Kubernetes provider supports `ModuleConfiguration`, `Ingress`, TLS secrets, and ingress annotations

## Provider Selection

The configuration provider is selected automatically based on the environment:

| Condition | Provider Used |
|----------|---------------|
| `KSBH__CONFIG_PATHS__CONFIG` is set | File-based |
| `KSBH__CONFIG_PATHS__CONFIG` is not set | Kubernetes |

```bash
# Use file-based configuration
export KSBH__CONFIG_PATHS__CONFIG="/app/config/config.yaml"

# Use Kubernetes configuration (default when variable is unset)
unset KSBH__CONFIG_PATHS__CONFIG
```

---

## File-Based Provider

The file-based provider watches and loads configuration from YAML files.

### How It Works

1. Reads configuration from the specified YAML file
2. Watches the file for changes using the `notify` crate
3. Automatically reloads configuration on modifications
4. Supports environment variable interpolation
5. supports ingress module attachment and exclusions
6. loads ingress TLS from `cert_file` and `key_file`
7. treats `secret_name` as metadata only in file mode

### Configuration File

Create a YAML file with your configuration:

```yaml
# /app/config/config.yaml
modules:
  - name: "rate-limiter"
    type: "ratelimit"
    weight: 100
    global: true
    config:
      score_threshold: "100"

ingresses:
  - name: "main"
    host: "example.com"
    paths:
      - path: "/"
        type: "prefix"
        backend: "service"
        service:
          name: "web"
          port: 80
```

### Enabling File Provider

```bash
export KSBH__CONFIG_PATHS__CONFIG="/app/config/config.yaml"
```

### Hot Reload Behavior

File changes trigger automatic reload. Supports modify and create events.

### Environment Variable Resolution

The file provider resolves environment variables in two ways:

1. **Direct interpolation** in YAML values:
   ```yaml
   config:
     issuer: "$OIDC_ISSUER"
   ```

2. **Regular env vars** for sensitive values:
   ```bash
   export KSBH__COOKIE_KEY="secret"
   ```

### Excluding Global Modules

The file provider supports ingress-level exclusion of global modules:

```yaml
ingresses:
  - name: "main"
    host: "example.com"
    modules:
      - "rate-limiter"
    excluded_modules:
      - "redirect-http"
    paths:
      - path: "/"
        type: "prefix"
        backend: "service"
        service:
          name: "web"
          port: 80
```

---

## Kubernetes Provider

The Kubernetes provider watches Custom Resources in a Kubernetes cluster.

### How It Works

1. Connects to Kubernetes API using in-cluster configuration or kubeconfig
2. Watches for `ModuleConfiguration` CRDs
3. Watches for `Ingress` resources with class `ksbh`
4. Automatically reconciles configuration changes
5. Manages TLS certificates from Secrets

### Prerequisites

- Kubernetes cluster with access to the API server
- RBAC permissions to read Ingress, Service, Secret, and ModuleConfiguration resources

### Custom Resources

#### ModuleConfiguration CRD

Defines module configurations:

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

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `spec.name` | string | Module name |
| `spec.type` | string | Module type (RateLimit, HttpToHttps, RobotsDotTXT, OIDC, POW, Custom) |
| `spec.weight` | integer | Higher values run earlier within the same scope |
| `spec.global` | boolean | Apply to all ingresses |
| `spec.secretRef` | object | Reference to Kubernetes Secret for config |
| `spec.requiresProperRequest` | boolean | Requires proper request headers |
| `spec.requiresBody` | boolean | Requires request body |

### Module Type Casing

The Kubernetes and file providers deserialize through the same module type enum. Prefer canonical names such as `RateLimit`, `HttpToHttps`, `RobotsDotTXT`, `OIDC`, `POW`, and `Custom`.

| Type | Kubernetes (exact) | File-based |
|------|-------------------|------------|
| `RateLimit` | `RateLimit` | aliases and case-insensitive variants accepted |
| `HttpToHttps` | `HttpToHttps` | aliases and case-insensitive variants accepted |
| `RobotsDotTXT` | `RobotsDotTXT` | aliases and case-insensitive variants accepted |
| `OIDC` | `OIDC` | aliases and case-insensitive variants accepted |
| `POW` | `POW` | aliases and case-insensitive variants accepted |
| `Custom` | `Custom` or custom type name | custom type name |

#### Ingress Resources

Standard Kubernetes Ingress with class `ksbh`:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: my-app
spec:
  ingressClassName: ksbh
  tls:
  - hosts:
    - example.com
    secretName: example-tls
  rules:
  - host: example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: web-service
            port:
              number: 80
```

Keep Kubernetes examples centered on `Ingress` plus `ModuleConfiguration` resources. Avoid relying on undocumented inline annotation patterns unless the controller behavior is explicitly verified.

### Supported Ingress Annotations

The Kubernetes provider currently reads:

| Annotation | Purpose |
|------------|---------|
| `ksbh.rs/modules` | Comma-separated module names attached to the ingress |
| `ksbh.rs/excluded-modules` | Comma-separated global module names to exclude for the ingress |

### TLS Certificate Management

The Kubernetes provider automatically loads TLS certificates from Secrets:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: example-tls
type: kubernetes.io/tls
data:
  # Base64 encoded
  tls.crt: LS0tLS1...
  tls.key: LS0tLS1...
```

Supported Secret formats:
- `tls.crt` / `tls.key` (PEM encoded)
- `stringData.tls.crt` / `stringData.tls.key` (plain text)

### Service Backend Types

The Kubernetes provider supports these backend types:

| Type | Kubernetes Backend | Description |
|------|-------------------|-------------|
| `service` | `IngressServiceBackend` | Kubernetes Service |
| `static` | `CrossNamespaceObjectReference` with kind `static` | Static content |
| `self` | `CrossNamespaceObjectReference` with kind `self` | Handle internally |

Example for static backend:

```yaml
apiVersion: service-resource.ksbh.rs/v1
kind: Static
metadata:
  name: static-content
---
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
      - path: /static
        pathType: Prefix
        backend:
          resource:
            apiGroup: service-resource.ksbh.rs
            kind: Static
            name: static-content
```

### Enabling Kubernetes Provider

Simply don't set the file path:

```bash
# Kubernetes provider will be used automatically
# No KSBH__CONFIG_PATHS__CONFIG set
```

---

## Comparing Providers

| Feature | File-Based | Kubernetes |
|---------|------------|------------|
| Configuration format | YAML | CRD + Ingress |
| Hot reload | Yes (file watch) | Yes (API watch) |
| Secrets management | Manual env vars | Kubernetes Secrets |
| Certificate management | Not currently implemented in the file provider | Auto from Secrets |
| Multi-tenant | Limited | Full |
| GitOps friendly | Yes | Yes (via ArgoCD, etc.) |
| Learning curve | Low | Medium |

---

## Provider Scope

Provider selection is exclusive at runtime:

- set `KSBH__CONFIG_PATHS__CONFIG` to use the file provider
- leave it unset to use the Kubernetes provider

If you want Git-managed configuration in Kubernetes, mount files into the pod and still use the file provider. Do not expect both providers to run at the same time.

One important caveat: provider selection and runtime config-file lookup are related but not identical concerns. Leaving `KSBH__CONFIG_PATHS__CONFIG` unset selects the Kubernetes provider, even though the runtime configuration model still has its own default `config_paths.config` value inside the loaded config structure.

---

## Switching Between Providers

### File → Kubernetes

1. Deploy the chart or apply the local CRDs from `charts/ksbh/crds/`
2. Convert `modules[]` to `ModuleConfiguration` resources
3. Convert ingresses to Ingress with class `ksbh`
4. Unset `KSBH__CONFIG_PATHS__CONFIG`

### Kubernetes → File

1. Export current configuration
2. Create YAML with same structure
3. Set `KSBH__CONFIG_PATHS__CONFIG`

---

## Troubleshooting

### Provider Selection Issues

```bash
# Check which provider is being used
# Look for logs:
# - File provider: "Watching config file: ..."
# - Kubernetes provider: "Started kubernetes controllers"
```

### File Provider Issues

```
ERROR: Failed to watch config file: ...
```

- Check file permissions
- Verify file path exists
- Ensure file is valid YAML

### Kubernetes Provider Issues

```
ERROR: Failed to connect to Kubernetes: ...
```

- Verify kubeconfig is correct
- Check RBAC permissions
- Ensure cluster is accessible

### Common RBAC Requirements

For the Kubernetes provider, ensure these RBAC rules:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: ksbh
rules:
- apiGroups: ["networking.k8s.io"]
  resources: ["ingresses"]
  verbs: ["get", "list", "watch"]
- apiGroups: [""]
  resources: ["services", "secrets"]
  verbs: ["get", "list", "watch"]
- apiGroups: ["modules.ksbh.rs"]
  resources: ["moduleconfigurations"]
  verbs: ["get", "list", "watch", "update", "patch"]
```
