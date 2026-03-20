pub(super) struct IngressCtx {
    pub(super) client: kube::Client,
    pub(super) certs: ksbh_core::certs::CertsWriter,
    pub(super) hosts: ksbh_core::routing::RouterWriter,
    pub(super) secret_refs: scc::HashMap<
        (ksbh_types::KsbhStr, ksbh_types::KsbhStr),
        kube::runtime::reflector::ObjectRef<k8s_openapi::api::networking::v1::Ingress>,
    >,
    pub(super) services_refs: scc::HashMap<
        (ksbh_types::KsbhStr, ksbh_types::KsbhStr),
        kube::runtime::reflector::ObjectRef<k8s_openapi::api::networking::v1::Ingress>,
    >,
}

#[derive(Debug)]
pub enum ReconcileError {
    KubeError(kube::Error),
    InvalidIngress(::std::string::String),
}

impl ::std::error::Error for ReconcileError {}

impl ::std::fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "ReconcileError: '{}'",
            match self {
                ReconcileError::KubeError(e) => e.to_string(),
                ReconcileError::InvalidIngress(e) => e.to_string(),
            }
        )
    }
}

impl From<kube::Error> for ReconcileError {
    fn from(value: kube::Error) -> Self {
        ReconcileError::KubeError(value)
    }
}

pub(super) async fn reconcile_ingress(
    obj: ::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
    ctx: ::std::sync::Arc<IngressCtx>,
) -> Result<kube::runtime::controller::Action, ReconcileError> {
    use kube::ResourceExt;

    let name = obj.name_any();
    let namespace = obj.namespace().unwrap_or("default".into());
    let api = kube::Api::<k8s_openapi::api::networking::v1::Ingress>::namespaced(
        ctx.client.clone(),
        &namespace,
    );

    if let Some(action) = handle_deletion(&obj, &api, &ctx, &name).await? {
        return Ok(action);
    }

    let spec = obj
        .spec
        .as_ref()
        .ok_or_else(|| ReconcileError::InvalidIngress("Missing spec for ingress".into()))?;

    if !is_valid_ingress_class(spec) {
        return Ok(kube::runtime::controller::Action::await_change());
    }

    let rules = spec
        .rules
        .clone()
        .ok_or_else(|| ReconcileError::InvalidIngress("No rules on ingress".into()))?;

    super::ensure_finalizer(&api, &*obj).await?;

    let module_names = extract_module_names(obj.metadata.annotations.as_ref());
    let excluded_modules = extract_excluded_module_names(obj.metadata.annotations.as_ref());

    process_tls_configs(ctx.as_ref(), &namespace, &name, spec.tls.as_ref()).await?;

    process_rules(
        ctx.as_ref(),
        &namespace,
        &name,
        &rules,
        &module_names,
        &excluded_modules,
        &obj,
    )
    .await?;

    Ok(kube::runtime::controller::Action::requeue(
        ::std::time::Duration::from_secs(250),
    ))
}

async fn handle_deletion(
    obj: &k8s_openapi::api::networking::v1::Ingress,
    api: &kube::Api<k8s_openapi::api::networking::v1::Ingress>,
    ctx: &IngressCtx,
    name: &str,
) -> Result<Option<kube::runtime::controller::Action>, ReconcileError> {
    use kube::Resource;

    if obj.meta().deletion_timestamp.is_some() {
        ctx.hosts.delete_ingress(name);
        super::remove_finalizer(api, obj).await?;

        return Ok(Some(kube::runtime::controller::Action::await_change()));
    }

    Ok(None)
}

fn is_valid_ingress_class(spec: &k8s_openapi::api::networking::v1::IngressSpec) -> bool {
    spec.ingress_class_name
        .as_ref()
        .is_some_and(|name| name == ksbh_core::constants::INGRESS_CLASS_NAME)
}

pub(crate) fn extract_module_names(
    annotations: Option<&std::collections::BTreeMap<::std::string::String, ::std::string::String>>,
) -> Vec<::std::sync::Arc<str>> {
    let mut module_names: Vec<::std::sync::Arc<str>> = Vec::new();

    if let Some(annotations) = annotations {
        for (key, value) in annotations {
            if key == ksbh_core::constants::KSBH_ANNOTATION_KEY_MODULES {
                let mut value = value.clone();
                ksbh_core::utils::remove_whitespace(&mut value);

                for module in value.split(",") {
                    module_names.push(::std::sync::Arc::from(module));
                }
            }
        }
    }

    module_names
}

pub(crate) fn extract_excluded_module_names(
    annotations: Option<&std::collections::BTreeMap<::std::string::String, ::std::string::String>>,
) -> Vec<::std::sync::Arc<str>> {
    let mut excluded_modules: Vec<::std::sync::Arc<str>> = Vec::new();

    if let Some(annotations) = annotations
        && let Some(value) =
            annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_EXCLUDED_MODULES)
    {
        let mut value = value.clone();
        ksbh_core::utils::remove_whitespace(&mut value);

        for module in value.split(",") {
            excluded_modules.push(::std::sync::Arc::from(module));
        }
    }

    excluded_modules
}

async fn process_tls_configs(
    ctx: &IngressCtx,
    namespace: &str,
    name: &str,
    tls_configs: Option<&Vec<k8s_openapi::api::networking::v1::IngressTLS>>,
) -> Result<(), ReconcileError> {
    let Some(tls_configs) = tls_configs else {
        return Ok(());
    };

    tracing::debug!(
        "Found {} TLS configs for ingress {}",
        tls_configs.len(),
        name
    );

    for tls in tls_configs {
        let Some(secret_name) = tls.secret_name.clone() else {
            continue;
        };

        let Some(secret) = fetch_tls_secret(ctx, namespace, &secret_name).await else {
            continue;
        };

        let (tls_crt, tls_key) = match parse_tls_secret(&secret, &secret_name) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse TLS secret {}/{}: {}",
                    namespace,
                    secret_name,
                    e
                );
                continue;
            }
        };

        let (private_key, certs) = match load_certificate(&tls_crt, &tls_key, &secret_name) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(
                    "Failed to load certificate from secret {}: {}",
                    secret_name,
                    e
                );
                continue;
            }
        };

        let (domains, wildcards) = extract_domains_from_cert(&certs);

        if let Err(e) = ctx
            .certs
            .add_cert(&secret_name, private_key, certs, domains, wildcards)
            .await
        {
            tracing::error!("Failed to add cert: {}", e);
        } else {
            tracing::info!("added cert");
        }
    }

    Ok(())
}

async fn fetch_tls_secret(
    ctx: &IngressCtx,
    namespace: &str,
    secret_name: &str,
) -> Option<k8s_openapi::api::core::v1::Secret> {
    let secret_api =
        kube::Api::<k8s_openapi::api::core::v1::Secret>::namespaced(ctx.client.clone(), namespace);

    let result = tokio::time::timeout(
        tokio::time::Duration::from_secs(3),
        secret_api.get(secret_name),
    )
    .await;

    match result {
        Ok(Ok(secret)) => Some(secret),
        Ok(Err(e)) => {
            tracing::warn!("Failed to get secret {}/{}: {}", namespace, secret_name, e);
            None
        }
        Err(e) => {
            tracing::error!(
                "TIMEOUT getting secret: {}.{}, {}",
                namespace,
                secret_name,
                e
            );
            None
        }
    }
}

pub(crate) fn parse_tls_secret(
    secret: &k8s_openapi::api::core::v1::Secret,
    secret_name: &str,
) -> Result<(::std::string::String, ::std::string::String), ::std::string::String> {
    let tls_crt = secret
        .data
        .as_ref()
        .and_then(|m| m.get("tls.crt").map(|b| b.0.clone()))
        .or_else(|| {
            secret
                .string_data
                .as_ref()
                .and_then(|m| m.get("tls.crt").map(|s| s.clone().into_bytes()))
        })
        .ok_or_else(|| format!("tls.crt not in secret {}", secret_name))?;

    let tls_crt = ::std::string::String::from_utf8(tls_crt)
        .map_err(|e| format!("Failed to convert tls_crt to utf8 string {}", e))?;

    let tls_key = secret
        .data
        .as_ref()
        .and_then(|m| m.get("tls.key").map(|b| b.0.clone()))
        .or_else(|| {
            secret
                .string_data
                .as_ref()
                .and_then(|m| m.get("tls.key").map(|s| s.clone().into_bytes()))
        })
        .ok_or_else(|| format!("tls.key not in secret {}", secret_name))?;

    let tls_key = ::std::string::String::from_utf8(tls_key)
        .map_err(|e| format!("Failed to convert tls_key to utf8 string {}", e))?;

    Ok((tls_crt, tls_key))
}

pub(crate) fn load_certificate(
    tls_crt: &str,
    tls_key: &str,
    secret_name: &str,
) -> Result<
    (
        pingora::tls::pkey::PKey<pingora::tls::pkey::Private>,
        Vec<pingora::tls::x509::X509>,
    ),
    ::std::string::String,
> {
    let private_key = pingora::tls::pkey::PKey::private_key_from_pem(tls_key.as_bytes())
        .map_err(|e| format!("Error when getting secret {}; '{}'", secret_name, e))?;

    let certs = pingora::tls::x509::X509::stack_from_pem(tls_crt.as_bytes())
        .map_err(|e| format!("Error when getting secret {}; '{}'", secret_name, e))?;

    Ok((private_key, certs))
}

pub(crate) fn extract_domains_from_cert(
    certs: &[pingora::tls::x509::X509],
) -> (Vec<ksbh_types::KsbhStr>, Vec<ksbh_types::KsbhStr>) {
    let mut wildcards: Vec<ksbh_types::KsbhStr> = Vec::new();
    let mut domains: Vec<ksbh_types::KsbhStr> = Vec::new();

    if let Some(leaf) = &certs.first()
        && let Some(sans) = &leaf.subject_alt_names()
    {
        for san in sans {
            if let Some(dns_name) = san.dnsname() {
                if dns_name.starts_with("*.") {
                    wildcards.push(ksbh_types::KsbhStr::new(dns_name));
                } else {
                    domains.push(ksbh_types::KsbhStr::new(dns_name));
                }
            }
        }
    }

    (domains, wildcards)
}

async fn process_rules(
    ctx: &IngressCtx,
    namespace: &str,
    name: &str,
    rules: &[k8s_openapi::api::networking::v1::IngressRule],
    module_names: &[::std::sync::Arc<str>],
    excluded_modules: &[::std::sync::Arc<str>],
    obj: &::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
) -> Result<(), ReconcileError> {
    let mut hosts: Vec<(::std::sync::Arc<str>, ksbh_core::routing::HostPaths)> = Vec::new();

    for rule in rules {
        let Some(ref http) = rule.http else {
            continue;
        };
        let Some(host) = &rule.host else {
            continue;
        };

        let mut host_paths = ksbh_core::routing::HostPaths::default();

        for path in &http.paths {
            let p = path.path.clone().unwrap_or_else(|| "/".into());

            let service = resolve_path_service(ctx, namespace, &path.backend, obj).await;

            match service {
                None => {}
                Some(service) => match path.path_type.to_lowercase().as_str() {
                    "exact" => {
                        host_paths
                            .exact
                            .insert(ksbh_types::KsbhStr::new(p), service);
                    }
                    "prefix" => host_paths
                        .prefix
                        .push((ksbh_types::KsbhStr::new(p), service)),
                    _ => host_paths
                        .implementation_specific
                        .push((ksbh_types::KsbhStr::new(p), service)),
                },
            }
        }

        hosts.push((::std::sync::Arc::from(host.clone()), host_paths));
    }

    let module_config = ksbh_core::routing::IngressModuleConfig {
        modules: module_names.to_vec(),
        excluded_modules: excluded_modules.to_vec(),
    };

    ctx.hosts.insert_ingress(name, hosts, module_config);

    Ok(())
}

pub(crate) async fn resolve_path_service(
    ctx: &IngressCtx,
    namespace: &str,
    backend: &k8s_openapi::api::networking::v1::IngressBackend,
    obj: &::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
) -> Option<ksbh_core::routing::ServiceBackendType> {
    if let Some(service) = &backend.service {
        let port = resolve_service_port(ctx, namespace, service).await?;

        let key = (
            ksbh_types::KsbhStr::new(namespace),
            ksbh_types::KsbhStr::new(&service.name),
        );
        ctx.services_refs
            .upsert_sync(key, kube::runtime::reflector::ObjectRef::from_obj(obj));

        let service_name = format!("{}.{}.svc.cluster.local", service.name, namespace);

        Some(ksbh_core::routing::ServiceBackendType::ServiceBackend(
            ksbh_core::routing::ServiceBackend {
                port,
                name: ksbh_types::KsbhStr::new(service_name),
            },
        ))
    } else if let Some(ressource) = &backend.resource {
        if ressource.api_group.as_ref().is_some_and(|api_group| {
            api_group == ksbh_core::constants::KSBH_K8S_SERVICE_RESSOURCE_API_GROUP
        }) {
            let ressource_kind = ressource.kind.to_lowercase();

            if ressource_kind == ksbh_core::constants::KSBH_SERVICE_RESSOURCE_KIND_STATIC {
                Some(ksbh_core::routing::ServiceBackendType::Static)
            } else if ressource_kind == ksbh_core::constants::KSBH_SERVICE_RESSOURCE_KIND_SELF {
                Some(ksbh_core::routing::ServiceBackendType::ToSelf(None))
            } else {
                Some(ksbh_core::routing::ServiceBackendType::None)
            }
        } else {
            Some(ksbh_core::routing::ServiceBackendType::None)
        }
    } else {
        Some(ksbh_core::routing::ServiceBackendType::None)
    }
}

async fn resolve_service_port(
    ctx: &IngressCtx,
    namespace: &str,
    service: &k8s_openapi::api::networking::v1::IngressServiceBackend,
) -> Option<u16> {
    if let Some(port) = service.port.as_ref().and_then(|p| p.number) {
        return Some(port as u16);
    }

    let port_name = service.port.as_ref().and_then(|p| p.name.as_ref())?;

    let svc_api: kube::Api<k8s_openapi::api::core::v1::Service> =
        kube::Api::namespaced(ctx.client.clone(), namespace);

    let svc = svc_api.get(&service.name).await.ok()?;

    svc.spec.and_then(|spec| {
        spec.ports.and_then(|ports| {
            ports
                .iter()
                .find(|p| p.name.as_deref() == Some(port_name))
                .map(|p| p.port as u16)
        })
    })
}

pub(super) fn error_ingress(
    obj: ::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
    err: &ReconcileError,
    _ctx: ::std::sync::Arc<IngressCtx>,
) -> kube::runtime::controller::Action {
    tracing::error!("Ingress: '{:?}', caused an error: '{}", obj, err);
    kube::runtime::controller::Action::requeue(::std::time::Duration::from_secs(60 * 5))
}
