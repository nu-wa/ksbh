+++
title = "Running"
+++

# Running KSBH

## Docker

Create `ksbh.yaml`:
```yaml
modules:
  - name: robots-txt
    type: robots.txt
    global: true
ingresses:
  - name: test
    host: localhost
    paths:
      - path: /
        type: prefix
        backend: static
```

Run:
```bash
docker run -d -p 8080:8080 -p 8081:8081 -p 8083:8083 -p 8084:8084 \
  -v $(pwd)/ksbh.yaml:/app/config/config.yaml:ro \
  -e KSBH__CONFIG_PATHS__CONFIG=/app/config/config.yaml \
  -e KSBH__REDIS_URL=redis://host.docker.internal:6379 \
  -e KSBH__COOKIE_KEY=$(openssl rand -base64 64) \
  ksbh:latest
```

## Standalone
```bash
cd crates && cargo build -p ksbh --release
openssl rand -base64 64  # cookie key
export KSBH__REDIS_URL=redis://127.0.0.1:6379
export KSBH__CONFIG_PATHS__CONFIG=./ksbh.yaml
export KSBH__COOKIE_KEY=<key>
./target/release/ksbh
```

**Ports**: 8080 HTTP, 8081 HTTPS, 8082 internal health, 8083 profiling, 8084 metrics
