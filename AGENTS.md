# KSBH Project

## Project Overview

KSBH is a Rust-based reverse proxy server built on top of [pingora](https://github.com/cloudflare/pingora), featuring a modular architecture with support for dynamically-loaded plugin modules via FFI, Kubernetes integration, Redis storage, and various request processing modules.

## Project Structure

```
crates/
├── ksbh-bin/                   # Main binary entry point
├── ksbh-core/                  # Core library functionality
├── ksbh-modules/               # Extensible plugin modules
│   ├── http_to_https/         # HTTP to HTTPS redirect module
│   ├── oidc/                  # OpenID Connect authentication
│   ├── proof-of-work/         # PoW challenge module
│   ├── rate-limit/            # Rate limiting module
│   └── robots-txt/            # robots.txt handling
├── ksbh-modules-sdk/          # SDK for building FFI modules
├── ksbh-config-providers/      # Configuration providers
│   ├── file/                  # File-based configuration
│   └── kubernetes/            # Kubernetes-based configuration
├── ksbh-types/                # Shared type definitions
└── tests/                     # Integration tests
```

## Key Dependencies

- **pingora** (0.7.0): Load balancing, caching, OpenSSL support
- **tokio** (1.49.0): Async runtime
- **kube** (2.0.1): Kubernetes client
- **libloading**: Dynamic library loading for FFI modules
- **redis** (1.0.2): Redis client for storage
- **async-trait**: For async trait methods
- **serde**: Serialization/deserialization
- **smol_str**: Memory-efficient string type

## Reading This Documentation

When working on this project with opencode:
1. **Always read the root AGENTS.md first** - it contains general conventions
2. **Read crate-specific AGENTS.md** when working in that crate - check `crates/<crate-name>/AGENTS.md`
3. **Check module AGENTS.md** when working on modules in `crates/ksbh-modules/<module>/AGENTS.md`

Opencode is configured to read these files, but you should also manually review them for crate-specific details.

## Documentation & Research Tools

When working on this project, you MUST use the available tools to look up documentation instead of assuming:

- **webfetch**: Use this to fetch specific documentation URLs (Rust docs, crate APIs, tutorials)
  - Example: `webfetch` tool to get https://docs.rs/tokio/latest/tokio/
- **context7**: Use this to search library documentation and tutorials
  - Add "use context7" to your prompt when searching for how to use a library
  - Example: "How do I use tokio::select? use context7"

**NEVER assume how an API works. Look it up first.**

## Common Mistakes

Avoid these frequent issues:

1. **Wrong Cargo Directory**: The workspace is in `crates/`, not the project root. Always use `-p <crate-name>` flag (e.g., `cargo test -p ksbh-core`)

2. **Wrong Package Name**: The main binary is named `ksbh`, not `ksbh-bin`. Use `-p ksbh` not `-p ksbh-bin`. Check Cargo.toml for actual package names.

3. **Unnecessary Compilation**: Do NOT run cargo check/build just for editing documentation. Only run verification commands when modifying actual code.

4. **Using `use` Imports**: Never use `use` statements. Always use full paths:
   - BAD: `use tokio::sync::mpsc;`
   - GOOD: `tokio::sync::mpsc`

3. **Using `unwrap()`/`expect()` in Production**: These crash the program. Always handle errors properly with `?` or `match`

4. **Skipping Verification**: Running `cargo check`, `cargo clippy`, and `cargo fmt` is NOT optional. Always verify before considering a task complete

5. **Assuming API Behavior**: Don't guess how a library works. Use webfetch or context7 to look it up

6. **Reinventing the Wheel**: Don't implement structs, functions, or logic from scratch. Search the existing codebase first:
   - Use `grep` to find similar implementations
   - Check `ksbh-types` for shared types
   - Check `ksbh-core` for core functionality (storage, routing, modules)
   - Check existing modules in `ksbh-modules/` for patterns
   - Look in relevant crate's `AGENTS.md` for available types

## Code Conventions

### 1. No `use` Imports - Full Path Required

Never use the Rust `use` keyword. Always use full path references:

```rust
// BAD
use std::fmt::Debug;
use std::collections::HashMap;
use tokio::sync::mpsc;

fn foo() -> HashMap<String, String> { ... }

// GOOD
::std::fmt::Debug
::std::collections::HashMap
tokio::sync::mpsc

fn foo() -> ::std::collections::HashMap<::std::string::String, ::std::string::String> { ... }
```

This convention is enforced throughout the codebase for consistency and clarity.

### 10. Collaborative Working

Always ask clarifying questions before making assumptions or decisions:
- Never assume functionality, behavior, or intent that isn't explicitly written in the code
- If you're unsure about something, ask the user instead of guessing
- Work collaboratively with the user, not autonomously
- When planning changes, present options and ask for confirmation before proceeding
- Don't make changes that weren't explicitly requested
- When confused or stuck, ask a question instead of looping on yourself

### 2. No unwrap() or expect() in Production Code

Never leave `.unwrap()` or `.expect()` in production code. Always handle results properly:

```rust
// BAD
let value = some_result.unwrap();
let config = option.expect("Config missing");

// GOOD - Use ? operator
let value = some_result?;

// GOOD - Explicit error handling
let value = match some_result {
    Ok(v) => v,
    Err(e) => {
        tracing::error!("Failed to get value: {}", e);
        return Err(MyError::ValueNotFound);
    }
};

// GOOD - Provide default
let value = option.unwrap_or(default_value);
```

### 3. Return Results by Default

When creating methods, always assume you'll need to return a `Result`:

```rust
// BAD - Assume no error handling needed
pub fn process_request(req: Request) -> ProcessedRequest {
    // ...
}

// GOOD - Return Result by default
pub fn process_request(req: Request) -> Result<ProcessedRequest, ProcessingError> {
    // ...
}
```

Only use non-Result returns when error handling is genuinely not needed (e.g., simple getters, constants).

### 4. Error Handling Patterns

Define errors as enums implementing `std::error::Error` and `std::fmt::Display`:

```rust
#[derive(Debug)]
pub enum MyError {
    InvalidInput(String),
    NetworkError(::std::io::Error),
    ConfigMissing,
}

impl ::std::error::Error for MyError {}

impl ::std::fmt::Display for MyError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::NetworkError(e) => write!(f, "Network error: {}", e),
            Self::ConfigMissing => write!(f, "Configuration missing"),
        }
    }
}

impl From<::std::io::Error> for MyError {
    fn from(value: ::std::io::Error) -> Self {
        Self::NetworkError(value)
    }
}
```

### 5. Naming Conventions

- **Types/Traits/Enums**: `PascalCase` (e.g., `ModuleRegistry`, `ConfigError`)
- **Functions/Methods**: `snake_case` (e.g., `get_config`, `process_request`)
- **Variables/Fields**: `snake_case` (e.g., `config_value`, `client_addr`)
- **Constants**: `SCREAMING_SNAKE_CASE` for const values
- **Modules**: `snake_case` (e.g., `proxy_apps`, `request_filter`)
- **Crates**: `kebab-case` in Cargo.toml, `snake_case` in imports

### 6. Module Organization

Follow this pattern for module organization:

```rust
// lib.rs or mod.rs
pub mod module_name;

// Use full paths for imports
pub use crate::module_name::{PublicType, PublicFunction};
```

becomes (for re-exports use full paths):

```rust
// In parent module
pub mod module_name;

mod parent {
    pub use super::module_name::{PublicType, PublicFunction};
}
```

### 7. Async Patterns

Use `async_trait` for async trait methods:

```rust
#[async_trait::async_trait]
pub trait Module: Send + Sync + ::std::fmt::Debug {
    async fn process(
        &self,
        session: &mut dyn Session,
    ) -> Result<bool, Box<dyn ModuleError>>;
}
```

### 8. Type Usage

- Use `smol_str::SmolStr` for short strings (less than ~24 chars)
- Use `::std::sync::Arc` for shared ownership
- Use `hashbrown::HashMap` instead of `std::collections::HashMap` for better performance
- Use `scc::HashMap` for concurrent hash maps
- Use custom newtype wrappers around primitives for type safety

### 9. Documentation

- Use doc comments (`///`) for public APIs
- Use module-level docs (`//!`) in mod.rs files
- Document error cases in method signatures

### 10. Testing

- Tests can use `expect()` for setup failures (test context)
- Use descriptive test names: `test_name_when_condition`
- Group related tests in modules

## Kubernetes Integration

The project uses Kubernetes Custom Resources for configuration:

```rust
#[derive(serde::Serialize, serde::Deserialize, kube::CustomResource, schemars::JsonSchema)]
#[kube(
    group = "modules.ksbh.rs",
    version = "v1",
    kind = "ModuleConfiguration",
    namespaced = false
)]
pub struct ModuleConfigurationSpec {
    pub name: String,
    pub r#type: ModuleConfigurationType,
    // ...
}
```

## Configuration Providers

The project supports multiple configuration sources via pluggable providers:

### File-based Configuration

Uses `ksbh-config-providers-file` crate:
- Watches YAML configuration files for changes
- Hot-reloads configuration without restart
- Uses `notify` crate for file system watching

### Kubernetes-based Configuration

Uses `ksbh-config-providers-kubernetes` crate:
- Loads configuration from Kubernetes Custom Resources
- Watches for configuration changes via Kubernetes API
- Supports `ModuleConfiguration` CRD

Both providers implement the `ConfigProvider` trait from `ksbh_core`:

```rust
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    async fn get_config(&self) -> Result<ProxyConfig, ConfigError>;
    async fn watch_config(&self) -> Result<Receiver<ProxyConfig>, ConfigError>;
}
```

## Module FFI System

The project uses dynamic libraries loaded at runtime via FFI (not Extism/WASM). The ABI is defined in `ksbh-core/src/modules/abi/`.

### How It Works

1. Modules are compiled as `cdylib` (dynamic libraries)
2. At startup, `ksbh-bin` loads modules using `libloading`
3. Modules expose a C-compatible FFI interface defined in the ABI

### ABI Components

Key types in `ksbh_core/src/modules/abi/`:
- `ModuleContext`: Request context passed to module (headers, config, body, etc.)
- `ResponseBuffer`: Host-provided buffer for module responses
- `ModuleEntryFn`: FFI entry point function type (`request_filter`)
- `KVSlice`: Key-value pair representation for headers/config
- `ModuleResultCode`: Result codes (Ok, Stop, Unauthorized, BadRequest, InternalError)

### Module Implementation Pattern

Use the `ksbh-modules-sdk` crate which provides a higher-level API:

```rust
fn handle_request(
    mut ctx: ksbh_modules_sdk::RequestContext<'_>,
) -> ksbh_modules_sdk::ModuleResult {
    // Access request info
    let path = ctx.request().path();
    
    // Process request and return Pass to continue, or Stop with response
    if some_condition {
        let response = http::Response::builder()
            .status(401)
            .body(bytes::Bytes::new())
            .unwrap();
        ksbh_modules_sdk::ModuleResult::Stop(response)
    } else {
        ksbh_modules_sdk::ModuleResult::Pass
    }
}

ksbh_modules_sdk::register_module!(handle_request, ksbh_core::modules::abi::ModuleTypeCode::Custom);
```

The SDK handles all FFI boilerplate automatically.

### RedisHashMap Helper

When storing module data, prefer using `RedisHashMap` from `ksbh_core::storage::RedisHashMap`. It provides:
- Hot (in-memory) cache with TTL
- Cold (Redis) fallback with persistence
- Automatic async watch for TTL expiration
- MessagePack serialization via `rmp-serde`

Example:
```rust
let cache: ksbh_core::storage::RedisHashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr> = 
    ksbh_core::storage::RedisHashMap::new(
        Some(ttl),           // hot cache TTL
        Some(redis_ttl),    // Redis persistence TTL
        Some(redis_connection),
    );
```

## OpenCode Configuration

The project includes an `opencode.json` configuration file located at the project root (`opencode.json`). This configures:

- **Permissions**: Security settings blocking dangerous commands (python, node, curl, docker, kubectl, git, etc.)
- **Bash**: Requires approval (`"ask"`) for most commands, with safe commands allowed (cargo, rustc, grep, ls, cat, make)
- **File watching**: Ignores build artifacts (`target/**`, `*.rs.bk`)

### Blocked Commands

The config ensures agents cannot:
- Run interpreters (python, node, ruby, etc.)
- Execute downloaded scripts (curl, wget)
- Use shell escapes (bash -c, sh -c, exec, eval)
- Use command patterns that bypass permissions (e.g., `cd ... && git ...`)
- Access git, docker, or kubectl
- Delete files (rm -rf)

### Allowed Commands

The following commands can be run without approval:
- `cargo` - Rust package manager
- `rustc` - Rust compiler
- `grep` - Text search
- `ls` - Directory listing
- `cat` - File reading

## Keeping AGENTS.md Updated

When making changes to the project that affect any of the following, you MUST update the relevant AGENTS.md file(s):

- Adding, removing, or renaming crates or modules
- Changing the module FFI interface or ABI
- Adding new dependencies that affect the build process
- Changing build, test, or lint commands
- Modifying code conventions or adding new ones
- Changes to project structure

Additionally:
- **Self-Correction**: If you make the same mistake twice, update AGENTS.md to prevent future repetition
- **Post-Task Reporting**: After completing any task, report what changes you made to AGENTS.md (if any) to help future sessions

Root AGENTS.md covers the whole project and general conventions.
Each crate has its own AGENTS.md for crate-specific details.

## Agent Workflow Requirements

When working on this project, agents MUST follow this workflow:

1. **Read relevant AGENTS.md first**:
   - Always read the root AGENTS.md
   - When entering a crate directory, read that crate's AGENTS.md
   - When working on a module, read the module's AGENTS.md

2. **Verify code compiles** (NOT OPTIONAL):
   - Run `cargo check` or `cargo build` before considering work complete
   - Run `cargo clippy -- -D warnings` to catch lint issues
   - Run `cargo fmt` to ensure code formatting is correct

3. **Run tests**:
   - Run `cargo test` for the relevant crate
   - Use `-- --nocapture` or `-- --show-output` when debugging

4. **Ask questions when uncertain**:
   - Never assume functionality or intent not in code
   - Ask user for clarification instead of guessing
   - Present options before making decisions

5. **Summarize conventions applied**:
   - At the end of every task, briefly list which code conventions you followed
   - Examples: full path imports, Result returns, error handling patterns

6. **Update AGENTS.md for repeated mistakes**:
   - If you make the same mistake twice, update the relevant AGENTS.md to prevent future repetition
   - Report what changes you made to AGENTS.md after the task

## Working Directory

The workspace is located at `/home/smspl/dev/rust/ksbh/crates/`. All paths referenced in documentation assume the working directory is `/home/smspl/dev/rust/ksbh/crates`.
