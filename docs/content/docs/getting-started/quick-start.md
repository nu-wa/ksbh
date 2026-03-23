+++
title = "Quick Start"
weight = 10
+++

# Quick Start

Get KSBH running in 5 minutes.

## 1. Start Redis (Optional)

```bash
redis-server --daemonize yes
```

## 2. Build

```bash
cd crates && cargo build -p ksbh --release
```

## 3. Generate Cookie Key

```bash
openssl rand -base64 64
```

## 4. Run

```bash
export KSBH__REDIS_URL=redis://127.0.0.1:6379
export KSBH__COOKIE_KEY='<paste key from step 3>'
./target/release/ksbh
```

Access: `http://localhost:8080` (HTTP), `http://localhost:8084/metrics` (metrics)

## 5. Verify

```bash
curl -I http://localhost:8080
curl http://localhost:8084/metrics
```
