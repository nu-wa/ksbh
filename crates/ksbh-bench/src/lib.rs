//! ksbh-bench: bench scenario rendering, analysis, comparison, and trending.

pub mod aggregate;
pub mod analyze;
pub mod compare;
pub mod render;
pub mod scenario;
pub mod template;
pub mod trend;

pub use scenario::{NginxOverrides, Scenario, ScenarioMeta, VegetaSpec};
