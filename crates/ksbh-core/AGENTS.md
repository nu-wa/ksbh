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

- `certs/`: Certificate management (CertsRegistry, CertsReader, CertsWriter, Certificate type)
- `config/`: Configuration structures (Config, ConfigPorts, ConfigListenAddresses, ConfigPerformance, ConfigFilePaths, ConfigURLPaths)
- `config_provider/`: ConfigProvider trait and ConfigService
- `constants/`: Application constants (DEFAULT_TCP_FASTOPEN_QUEUE_SIZE, etc.)
- `cookies/`: Cookie encryption/decryption
- `metrics/`: Prometheus metrics
- `modules/abi/`: FFI interface definitions - error.rs, host_functions.rs, log.rs, macros.rs, module_buffer.rs, module_context.rs, module_host.rs, module_instance.rs, module_request_context.rs, module_request_info.rs, module_response.rs
- `modules/`: Plugin system with registry (ModuleRegistry) loading dynamic libraries at runtime via `libloading`
- `proxy/`: Core proxy implementation (service.rs, service_request_filter.rs, test_utils.rs)
- `routing/`: Request routing (hosts.rs, path_type.rs, request_match.rs, router.rs, service_backend.rs)
- `storage/`: Redis storage abstraction (module_session_key.rs, redis_hashmap.rs)
- `utils/`: Utility functions (get_env, get_env_prefer_file, current_unix_time, watch_directory_files_async, get_client_ip_from_session, etc.)

## Module FFI System

Modules are compiled as `cdylib` (dynamic libraries) and loaded at runtime. The ABI is defined in `modules/abi/`:

- `ModuleContext`: Request context passed to module entry point
- `ResponseBuffer`: Host-provided buffer for module responses  
- `ModuleEntryFn`: FFI entry point function type (`request_filter`)
- `KVSlice`: Key-value pair representation for headers/config

Modules must export two FFI functions: `request_filter` and `get_module_type`.

See root `AGENTS.md` for detailed module implementation pattern.

## ConfigProvider Trait

```rust
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn start(
        &self,
        router: RouterWriter,
        certs: CertsWriter,
        shutdown: tokio::sync::watch::Receiver<bool>,
    );
}
```

The `ConfigService` wraps a `ConfigProvider` and implements `pingora::services::background::BackgroundService`.

## Constants

Key constants in `constants.rs`:
- `DEFAULT_TCP_FASTOPEN_QUEUE_SIZE`: Default TFO queue size
- `REDIS_STATS_KEY`: Redis key for stats
- `PLUGIN_ENTRYPOINT`: Module FFI entry point name ("request_filter")
- `ENV_KSBH_COOKIE_KEY`: Environment variable for cookie encryption key
- `ENV_KSBH_SESSION_COOKIE_NAME`: Environment variable for session cookie name

## Utility Functions

Key utilities in `utils/mod.rs`:
- `get_env(key)`: Get environment variable (supports `_FILE` suffix)
- `get_env_prefer_file(key)`: Prefer reading from file if `_<key>_FILE` exists
- `current_unix_time()`: Get current Unix timestamp
- `watch_directory_files_async()`: Async directory watching with notify
- `get_client_ip_from_session()`: Extract client IP from session headers

## RedisHashMap Helper

When storing module data, prefer using `RedisHashMap` from `ksbh_core::storage::RedisHashMap`. It provides:
- Hot (in-memory) cache with TTL
- Cold (Redis) fallback with persistence
- Automatic async watch for TTL expiration
- MessagePack serialization via `rmp-serde`

## Public Exports

The crate re-exports the following:
- `notify`, `walkdir`, `cookie` crates
- `RedisProvider`, `Storage` from storage module
- `RedisHashMap` from storage module
- `COOKIE_ENC_KEY`: Lazy-initialized cookie encryption key from env
- `COOKIE_NAME`: Lazy-initialized cookie name from env (default: "ksbh")

## Key Dependencies

- `pingora`: HTTP framework
- `redis`: Storage backend
- `kube`: Kubernetes integration
- `notify`: File system watching
- `walkdir`: Directory traversal

## Build

```bash
cargo build -p ksbh-core
cargo test -p ksbh-core
```

## Conventions

Follow the general conventions in the root `AGENTS.md`.
