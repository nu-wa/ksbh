pub mod hosts;
pub mod ingress;
pub mod ingress_recipe;
pub mod path_type;
pub mod request_match;
pub mod router;
pub mod service_backend;

pub use hosts::{GlobalConfig, HostConfiguration, HostPaths, HostRegistry};
pub use ingress_recipe::{
    Backend, HostRules, IngressData, IngressRecipe, IngressRecipeError, IngressRule, ModuleData,
    TlsData,
};
pub use path_type::PathType;
pub use request_match::RequestMatch;
pub use router::{
    IngressModuleConfig, Router, RuntimeIngressSnapshot, RuntimeModuleSnapshot,
    RuntimeStateSnapshot, router_reader::RouterReader, router_writer::RouterWriter,
};
pub use service_backend::{Upstream, RoutingDestination};

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod tests;
