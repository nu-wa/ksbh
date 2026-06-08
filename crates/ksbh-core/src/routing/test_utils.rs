//! Routing test harness — `#[cfg(test)]` only.
//!
//! Provides a small, ergonomic surface for writing focused unit tests against
//! the router. Tests use [`RouterHarness`] to populate the router via
//! `Router::create()` + `RouterWriter::{insert_ingress, upsert_module}` and
//! to query it via `RouterReader::find_route`.
//!
//! This module is compiled only in test builds and adds no new public types
//! to the release artifact.

#![cfg(test)]

use ::std::sync::Arc;

use http::Method;
use ksbh_types::prelude::{HttpQuery, HttpRequest};
use ksbh_types::KsbhStr;

use crate::modules::{ModuleConfigurationSpec, ModuleConfigurationType};
use crate::routing::{
    HostPaths, IngressModuleConfig, Router, RouterReader, RouterWriter, RoutingDestination,
};

/// Builds a `HostPaths` with chained configuration calls. Keeps test fixtures
/// focused on the assertion, not the field shape.
#[derive(Default)]
pub struct HostPathsBuilder {
    paths: HostPaths,
}

impl HostPathsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn exact(mut self, path: &str, backend: RoutingDestination) -> Self {
        self.paths
            .exact
            .insert(KsbhStr::new(path), backend);
        self
    }

    pub fn prefix(mut self, path: &str, backend: RoutingDestination) -> Self {
        self.paths
            .prefix
            .push((KsbhStr::new(path), backend));
        self
    }

    pub fn implementation_specific(
        mut self,
        path: &str,
        backend: RoutingDestination,
    ) -> Self {
        self.paths
            .implementation_specific
            .push((KsbhStr::new(path), backend));
        self
    }

    pub fn build(self) -> HostPaths {
        self.paths
    }
}

/// Constructs a minimal `ModuleConfigurationSpec` suitable for tests. Avoids
/// making each test spell out every field of the spec struct.
pub fn test_module_spec(name: &str, weight: i32, global: bool) -> ModuleConfigurationSpec {
    ModuleConfigurationSpec {
        name: name.to_string(),
        r#type: ModuleConfigurationType::Custom(name.to_string()),
        weight,
        global,
        secret_ref: None,
        config: None,
        requires_body: false,
    }
}

/// Focused unit-test harness for the router.
///
/// Internally owns a `RouterReader` and `RouterWriter` pair produced by
/// `Router::create()`. Each `add_host*` call creates a fresh ingress so
/// multiple hosts in the same test get isolated module chains.
pub struct RouterHarness {
    reader: RouterReader,
    writer: RouterWriter,
    next_ingress_id: u64,
}

impl RouterHarness {
    /// Create a new harness backed by a freshly constructed router.
    pub fn new() -> Self {
        let (reader, writer) = Router::create();
        Self {
            reader,
            writer,
            next_ingress_id: 0,
        }
    }

    /// Register a host with the given paths. The host is bound to a new
    /// ingress with no module chain.
    pub fn add_host(&mut self, host: &str, paths: HostPaths) -> &mut Self {
        let ingress = self.next_ingress_name();
        self.writer.insert_ingress(
            &ingress,
            vec![(Arc::from(host), paths)],
            IngressModuleConfig::default(),
            None,
        );
        self
    }

    /// Register a host whose ingress has the given module chain. The named
    /// modules must already exist in the (non-global) module registry —
    /// register them with [`RouterHarness::add_module`] first.
    pub fn add_host_with_modules(
        &mut self,
        host: &str,
        paths: HostPaths,
        modules: Vec<Arc<str>>,
    ) -> &mut Self {
        let ingress = self.next_ingress_name();
        self.writer.insert_ingress(
            &ingress,
            vec![(Arc::from(host), paths)],
            IngressModuleConfig {
                modules,
                excluded_modules: vec![],
            },
            None,
        );
        self
    }

    /// Register a non-global module in the module registry. Pair it with
    /// [`RouterHarness::add_host_with_modules`] to attach it to an ingress.
    pub fn add_module(&mut self, name: &str, spec: ModuleConfigurationSpec) -> &mut Self {
        self.writer
            .upsert_module(name, false, Arc::new(hashbrown::HashMap::new()), spec);
        self
    }

    /// Register a global module. Global modules are included in every
    /// `RequestMatch` produced by the router, sorted by weight desc / name asc.
    pub fn add_global_module(&mut self, name: &str, spec: ModuleConfigurationSpec) -> &mut Self {
        self.writer
            .upsert_module(name, true, Arc::new(hashbrown::HashMap::new()), spec);
        self
    }

    /// Build a `GET /<path>` request against `<host>` and route it through
    /// the reader. Returns whatever the router would hand back to the proxy.
    pub fn find_route(&self, host: &str, path: &str) -> Option<crate::routing::RequestMatch> {
        self.reader.find_route(&build_get_request(host, path))
    }

    fn next_ingress_name(&mut self) -> String {
        let id = self.next_ingress_id;
        self.next_ingress_id += 1;
        format!("test-ingress-{id}")
    }
}

impl Default for RouterHarness {
    fn default() -> Self {
        Self::new()
    }
}

fn build_get_request(host: &str, path: &str) -> HttpRequest {
    let base_url = format!("http://{host}");
    let uri = format!("{base_url}{path}");

    HttpRequest {
        uri: KsbhStr::new(uri),
        base_url: KsbhStr::new(base_url),
        host: KsbhStr::new(host),
        port: 80,
        query: HttpQuery {
            path: KsbhStr::new(path),
            params: vec![],
        },
        scheme: http::uri::Scheme::HTTP,
        req_uuid: uuid::Uuid::nil(),
        method: Method::GET,
    }
}
