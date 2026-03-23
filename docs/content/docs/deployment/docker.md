+++
title = "Docker Deployment"
weight = 31
path = "/deployment/docker/"
+++

# Docker Deployment

## Building the Image

The repo-local way to build the release image is:

```bash
mise run build-release-image
```

If you want to call Docker directly, build from the repo root with `docker/build/release.Dockerfile`.

## Important Environment Variables

| Variable | Description |
|----------|-------------|
| `KSBH__CONFIG_PATHS__CONFIG` | Path to the YAML config file when using the file provider |
| `KSBH__CONFIG_PATHS__MODULES` | Directory containing dynamic modules |
| `KSBH__COOKIE_KEY` | Cookie encryption key |
| `KSBH__REDIS_URL` | Optional Redis URL |
| `DEBUG_LEVEL` | Log filter string |

## Basic Run

```bash
docker run -d \
  --name ksbh \
  -p 8080:8080 \
  -p 8081:8081 \
  -p 8083:8083 \
  -p 8084:8084 \
  -v $(pwd)/ksbh.yaml:/app/config/config.yaml:ro \
  -e KSBH__CONFIG_PATHS__CONFIG=/app/config/config.yaml \
  -e KSBH__REDIS_URL=redis://host.docker.internal:6379/0 \
  -e KSBH__COOKIE_KEY="$(openssl rand -base64 64)" \
  ksbh:latest
```

## Minimal Compose Example

```yaml
services:
  ksbh:
    image: ksbh:latest
    ports:
      - "8080:8080"
      - "8081:8081"
      - "8083:8083"
      - "8084:8084"
    volumes:
      - ./ksbh.yaml:/app/config/config.yaml:ro
      - ./modules:/app/modules:ro
    environment:
      KSBH__CONFIG_PATHS__CONFIG: /app/config/config.yaml
      KSBH__REDIS_URL: redis://redis:6379/0
      KSBH__COOKIE_KEY: ${KSBH__COOKIE_KEY}
      DEBUG_LEVEL: INFO
    depends_on:
      - redis

  redis:
    image: redis:7-alpine
```

## Container Layout

The release image currently uses this layout:

| Path | Purpose |
|------|---------|
| `/app/ksbh` | KSBH binary |
| `/app/config` | Runtime config directory |
| `/app/modules` | Dynamic module directory |
| `/app/data/static` | Static content directory |

## Notes

- The current docs only guarantee the local Dockerfile and the `mise` task, not a published image registry workflow.
- Redis is optional for startup, but modules and features that depend on shared state work better with it configured.
