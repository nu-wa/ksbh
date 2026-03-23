+++
title = "Getting Started"
description = "Install and configure KSBH reverse proxy"
weight = 10
+++

# Getting Started

Get KSBH installed and running in your environment.

## Prerequisites

- **Redis**: Optional. Some features use it for session storage and scoring.
- **Rust toolchain with edition 2024 support**: Required if building from source
- **libssl-dev**, **pkg-config**, **build-essential**, **cmake**: System libraries for building

## Installation

- [Installation Guide](/docs/getting-started/installation/) - Helm, Docker, or build from source
- [Quick Start](/docs/getting-started/quick-start/) - Get running in 5 minutes
- [Running Guide](/docs/getting-started/running/) - Full runtime documentation

## Quick Reference

### Environment

- `KSBH__COOKIE_KEY`: required when you provide the cookie key through the environment
- `KSBH__CONFIG_PATHS__CONFIG`: only needed when using the file provider
- `KSBH__REDIS_URL`: optional

### Default Ports

| Port | Purpose |
|------|---------|
| `8080` | HTTP |
| `8081` | HTTPS |
| `8082` | Internal health |
| `8083` | Profiling |
| `8084` | Metrics |
