//! Kubernetes-based configuration provider for KSBH.
//!
//! Loads configuration from Kubernetes Custom Resources (ModuleConfiguration CRD)
//! and watches for changes via the Kubernetes API.

pub mod provider;

pub use provider::KubeConfigProvider;
