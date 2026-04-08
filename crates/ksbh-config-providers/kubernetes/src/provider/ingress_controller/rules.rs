pub async fn parse_rules(
    ctx: &super::IngressController,
    namespace: &str,
    name: &str,
    rules: &[k8s_openapi::api::networking::v1::IngressRule],
    annotations: super::annotations::Annotations,
    obj: &::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
) -> Result<(), crate::provider::controller_error::ControllerError> {
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
        modules: annotations.modules,
        excluded_modules: annotations.excluded_modules,
    };

    ctx.hosts
        .insert_ingress(name, hosts, module_config, annotations.peer_options);

    Ok(())
}

pub(crate) async fn resolve_path_service(
    ctx: &super::IngressController,
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
            } else if ressource_kind == "self" {
                tracing::warn!(
                    "Ingress '{}/{}' path resource kind 'self' is deprecated; treating as no backend",
                    namespace,
                    obj.metadata.name.as_deref().unwrap_or("<unknown>")
                );
                Some(ksbh_core::routing::ServiceBackendType::None)
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
    ctx: &super::IngressController,
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
