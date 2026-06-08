//! ksbh-bench: bench scenario rendering, analysis, comparison, and trending.
//!
//! The [`runner`] module replaces `bench/run-scenario.sh` — one async function
//! call runs a full benchmark lifecycle (containers, certs, vegeta, metrics,
//! RSS, analysis) behind a typed interface.

pub mod aggregate;
pub mod analyze;
pub mod compare;
pub mod render;
pub mod runner;
pub mod scenario;
pub mod template;
pub mod trend;

pub use runner::{Mode, ProxyKind, ProxyResult, RunConfig, ScenarioOutput};
pub use scenario::{ModuleBenchSpec, NginxOverrides, Scenario, ScenarioMeta, VegetaSpec};
