//! Benchmark lifecycle runner — replaces `bench/run-scenario.sh`.
//!
//! # Design
//!
//! Two public entry points: [`run`] and [`gate`]. Everything else is hidden
//! behind `#[cfg(test)]` seams in submodules — no traits, no generics, no DI
//! in the public interface. Callers see two async functions and four types.
//!
//! # Internal modules (each has real + test implementations)
//!
//! - `ports` — free-port allocation (bind-then-release)
//! - `certs` — self-signed cert generation via `rcgen`
//! - `upstream` — upstream container lifecycle via `testcontainers`
//! - `nginx` — nginx container lifecycle via `testcontainers`
//! - `ksbh_process` — host-process spawn/kill/signal
//! - `vegeta` — subprocess invocation (warmup + attack + report)
//! - `metrics` — Prometheus scraper (background task)
//! - `rss` — RSS sampler (ps for host, docker exec for container)
//! - `signal` — cleanup guard + ctrlc handler

use std::path::PathBuf;

use anyhow::Result;

use crate::analyze::ResultDoc;

// ── Public types ────────────────────────────────────────────────────

/// Determines which proxies are run and what the output compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Run only ksbh. No nginx, no module. Fast CI pre-merge gate.
    KsbhOnly,
    /// Run ksbh then nginx against the same upstream and scenario.
    KsbhVsNginx,
    /// Run ksbh twice: first without module (baseline), then with the
    /// module library loaded and configured. Measures module overhead.
    KsbhModule,
}

/// Configuration for [`run`].
///
/// Every field has a sensible default except `mode`. The CLI constructs
/// this directly; there is no builder.
pub struct RunConfig {
    /// Which proxies to run and what to compare.
    pub mode: Mode,

    /// Where to write `{scenario}_{proxy}.json` result files.
    /// Defaults to `bench/results`.
    pub out_dir: PathBuf,

    /// Path to the ksbh binary. Auto-discovered if not set:
    /// `KSBH_BIN` env → `crates/target/debug/ksbh` → `crates/target/release/ksbh`.
    pub ksbh_bin: Option<PathBuf>,

    /// Path to the vegeta binary. Auto-discovered if not set:
    /// `vegeta` on PATH.
    pub vegeta_bin: Option<PathBuf>,

    /// Path to the module cdylib (`.so` / `.dylib`). Required when
    /// `mode` is `KsbhModule`.
    pub module_lib: Option<PathBuf>,
}

/// All results from one [`run`] invocation.
#[derive(Debug, Clone)]
pub struct ScenarioOutput {
    pub scenario_id: String,
    /// One entry per proxy that was run:
    /// - `KsbhOnly`    → 1
    /// - `KsbhVsNginx` → 2 (ksbh, nginx)
    /// - `KsbhModule`  → 2 (ksbh-baseline, ksbh-{module})
    pub results: Vec<ProxyResult>,
}

/// One proxy's worth of benchmark data.
#[derive(Debug, Clone)]
pub struct ProxyResult {
    pub proxy: ProxyKind,
    /// The analysed result document (existing type from `analyze.rs`).
    pub result_doc: ResultDoc,
    /// Absolute path to the written JSON file on disk.
    pub result_path: PathBuf,
}

/// Labels a proxy in the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyKind {
    Ksbh,
    Nginx,
    KsbhModule { module: String },
}

impl std::fmt::Display for ProxyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyKind::Ksbh => write!(f, "ksbh"),
            ProxyKind::Nginx => write!(f, "nginx"),
            ProxyKind::KsbhModule { module } => write!(f, "ksbh-module-{}", module),
        }
    }
}

// ── Public entry points ─────────────────────────────────────────────

/// Run a full benchmark scenario end-to-end.
///
/// Loads the scenario TOML from `bench/scenarios/{name}.toml`, allocates
/// ports, generates a self-signed cert, renders configs, starts containers
/// (testcontainers) and the ksbh host process, runs vegeta against each
/// proxy, scrapes metrics and RSS, produces [`ResultDoc`] files, and
/// cleans up on all exit paths (success, error, SIGTERM, SIGINT).
///
/// The caller (CLI or test) receives a [`ScenarioOutput`] — no file paths,
/// no port numbers, no PIDs.
pub async fn run(name: &str, config: RunConfig) -> Result<ScenarioOutput> {
    let _ = (name, config); // stub — implementation in follow-up
    unimplemented!("runner::run is not yet implemented")
}

/// CI gate: run ksbh-only and return an exit code.
///
/// Shortcut for `run(name, RunConfig { mode: Mode::KsbhOnly, .. })`
/// followed by the existing [`crate::aggregate::gate`] logic.
///
/// Returns `0` if all speed scenarios pass; `1` if any fail (rps == 0
/// or error rate > 10%).
pub async fn gate(name: &str) -> i32 {
    let _ = name; // stub — implementation in follow-up
    unimplemented!("runner::gate is not yet implemented")
}

// ── Submodules (stubs — implemented in follow-up PRs) ────────────────
//
// mod ports;
// mod certs;
// mod upstream;
// mod nginx;
// mod ksbh_process;
// mod vegeta;
// mod metrics;
// mod rss;
// mod signal;
