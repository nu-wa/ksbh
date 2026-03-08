# ksbh-core

Core library providing the fundamental building blocks for the KSBH reverse proxy.

## Purpose

This crate provides:
- **Modules**: Dynamic library plugin system with FFI interface for request processing
- **Proxy**: Core proxy logic and request handling
- **Routing**: Path-based and host-based routing
- **Storage**: Redis-based storage providers
- **Metrics**: Prometheus metrics collection
- **Cookies**: Secure cookie handling

## Key Components

- `modules/abi/`: FFI interface definitions (C-compatible types for dynamic libraries)
- `modules/`: Plugin system loading dynamic libraries at runtime via `libloading`
- `proxy/`: Core proxy implementation
- `routing/`: Request routing (hosts, paths, backends)
- `storage/`: Redis storage abstraction
- `metrics/`: Prometheus metrics
- `cookies/`: Cookie encryption/decryption

## Module FFI System

Modules are compiled as `cdylib` (dynamic libraries) and loaded at runtime. The ABI is defined in `modules/abi/`:

- `ModuleContext`: Request context passed to module entry point
- `ResponseBuffer`: Host-provided buffer for module responses  
- `ModuleEntryFn`: FFI entry point function type (`request_filter`)
- `KVSlice`: Key-value pair representation for headers/config

Modules must export two FFI functions: `request_filter` and `get_module_type`.

See root `AGENTS.md` for detailed module implementation pattern.

## RedisHashMap Helper

When storing module data, prefer using `RedisHashMap` from `ksbh_core::storage::RedisHashMap`. It provides:
- Hot (in-memory) cache with TTL
- Cold (Redis) fallback with persistence
- Automatic async watch for TTL expiration
- MessagePack serialization via `rmp-serde`

## Key Dependencies

- `pingora`: HTTP framework
- `redis`: Storage backend
- `kube`: Kubernetes integration

## Build

```bash
cargo build -p ksbh-core
cargo test -p ksbh-core
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
