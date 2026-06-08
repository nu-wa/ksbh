//! Shared ingress-to-router/certs emitter.
//!
//! The file-based and Kubernetes config providers both translate "an ingress"
//! into the same low-level calls against the [`crate::routing::RouterWriter`]
//! and the [`crate::certs::CertsWriter`]. That translation lives here so the
//! two adapters can stop re-implementing it.
//!
//! Each adapter converts its native shape (a YAML file, a `k8s_openapi`
//! `Ingress` object) into an [`IngressData`] — a transport-agnostic
//! description of "what should be installed" — and then calls
//! [`IngressRecipe::emit`].

use thiserror::Error;

use crate::certs::CertsWriter;
use crate::modules::{ModuleConfigurationSpec, ModuleConfigurationValues};
use crate::routing::router::router_writer::RouterWriter;
use crate::routing::{HostPaths, IngressModuleConfig, PathType, RoutingDestination, Upstream};
use ksbh_types::KsbhStr;

/// How a request should be routed once a path matches.
#[derive(Debug, Clone)]
pub enum Backend {
    /// Forward to a named upstream (host:port).
    Upstream { name: KsbhStr, port: u16 },
    /// Serve a static response.
    Static,
    /// Return an error response with a static message.
    Error(&'static str),
    /// No destination configured — matches but does not route.
    None,
}

impl Backend {
    fn into_routing_destination(self) -> RoutingDestination {
        match self {
            Self::Upstream { name, port } => {
                RoutingDestination::Upstream(Upstream { name, port })
            }
            Self::Static => RoutingDestination::Static,
            Self::Error(msg) => RoutingDestination::Error(msg),
            Self::None => RoutingDestination::None,
        }
    }
}

/// All the path → backend rules that bind to a single host.
#[derive(Debug, Clone)]
pub struct HostRules {
    pub host: String,
    pub rules: Vec<IngressRule>,
}

/// A single path → backend rule that will become a [`HostPaths`] entry.
#[derive(Debug, Clone)]
pub struct IngressRule {
    pub path: String,
    pub path_type: PathType,
    pub backend: Backend,
}

/// TLS material to load into the certs registry.
#[derive(Debug, Clone)]
pub struct TlsData {
    /// Logical name the cert will be registered under (typically the
    /// ingress name in the file provider or the secret name in the K8s
    /// provider).
    pub name: String,
    pub cert_pem: String,
    pub key_pem: String,
}

/// A module to upsert into the router's module registry.
#[derive(Debug, Clone)]
pub struct ModuleData {
    pub name: String,
    pub spec: ModuleConfigurationSpec,
    pub values: ModuleConfigurationValues,
}

/// The transport-agnostic shape of "an ingress to install": a set of
/// host-bound rules, optional TLS material, an attached module chain, and
/// zero or more modules to register in the module registry first.
#[derive(Debug, Clone)]
pub struct IngressData {
    /// Logical ingress name. Maps to the `name` argument of
    /// [`RouterWriter::insert_ingress`].
    pub name: String,
    /// One entry per host. The file provider typically has exactly one;
    /// the K8s provider emits one per `IngressRule` with a non-empty host.
    pub hosts: Vec<HostRules>,
    pub tls: Vec<TlsData>,
    /// Modules to attach to this ingress (the `IngressModuleConfig.modules`
    /// list — i.e. the ingress-scoped chain).
    pub attached_modules: Vec<String>,
    /// Modules to exclude from the global chain for this ingress.
    pub excluded_modules: Vec<String>,
    pub peer_options: Option<ksbh_types::providers::proxy::peer_options::PeerOptions>,
    /// Modules to upsert into the router's module registry before the
    /// ingress is inserted. The file provider emits one of these per
    /// `FileConfigModules`; the K8s adapter does not — its
    /// [`crate::modules::ModuleConfiguration`] controller handles module
    /// upserts directly.
    pub modules: Vec<ModuleData>,
}

#[derive(Debug, Error)]
pub enum IngressRecipeError {
    #[error("failed to load PEM cert {name}: {source}")]
    PemLoad {
        name: String,
        #[source]
        source: Box<dyn ::std::error::Error + 'static>,
    },
}

/// Translate an [`IngressData`] into router + certs registry calls.
///
/// This is the single source of truth for "given an ingress, install it".
/// It is the only function in the codebase that should call both
/// [`RouterWriter::insert_ingress`] and the certs registry (via
/// [`crate::certs::load_pem_into_registry`]) for the same ingress.
pub struct IngressRecipe;

impl IngressRecipe {
    pub async fn emit(
        data: &IngressData,
        router: &mut RouterWriter,
        certs: &mut CertsWriter,
    ) -> Result<(), IngressRecipeError> {
        for module in &data.modules {
            router.upsert_module(
                &module.name,
                module.spec.global,
                module.values.clone(),
                module.spec.clone(),
            );
        }

        for tls in &data.tls {
            crate::certs::load_pem_into_registry(
                certs,
                &tls.name,
                tls.cert_pem.as_bytes(),
                tls.key_pem.as_bytes(),
            )
            .await
            .map_err(|source| IngressRecipeError::PemLoad {
                name: tls.name.clone(),
                source,
            })?;
        }

        let mut hosts: Vec<(::std::sync::Arc<str>, HostPaths)> = Vec::new();

        for host_rules in &data.hosts {
            let mut host_paths = HostPaths::default();

            for rule in &host_rules.rules {
                let key = KsbhStr::new(&rule.path);
                let backend = rule.backend.clone().into_routing_destination();

                match rule.path_type {
                    PathType::Exact => {
                        host_paths.exact.insert(key, backend);
                    }
                    PathType::Prefix => {
                        host_paths.prefix.push((key, backend));
                    }
                    PathType::ImplementationSpecific => {
                        host_paths.implementation_specific.push((key, backend));
                    }
                }
            }

            hosts.push((::std::sync::Arc::from(host_rules.host.as_str()), host_paths));
        }

        let module_config = IngressModuleConfig {
            modules: data
                .attached_modules
                .iter()
                .map(|s| ::std::sync::Arc::from(s.as_str()))
                .collect(),
            excluded_modules: data
                .excluded_modules
                .iter()
                .map(|s| ::std::sync::Arc::from(s.as_str()))
                .collect(),
        };

        router.insert_ingress(&data.name, hosts, module_config, data.peer_options.clone());

        Ok(())
    }
}
