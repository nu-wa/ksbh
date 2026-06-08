pub async fn collect_rules(
    ctx: &super::IngressController,
    namespace: &str,
    rules: &[k8s_openapi::api::networking::v1::IngressRule],
    obj: &::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
) -> Result<Vec<ksbh_core::routing::HostRules>, crate::provider::controller_error::ControllerError> {
    let mut hosts: Vec<ksbh_core::routing::HostRules> = Vec::new();

    for rule in rules {
        let Some(ref http) = rule.http else {
            continue;
        };
        let Some(host) = &rule.host else {
            continue;
        };

        let mut rules_out: Vec<ksbh_core::routing::IngressRule> = Vec::new();

        for path in &http.paths {
            let p = path.path.clone().unwrap_or_else(|| "/".into());

            let service = resolve_path_service(ctx, namespace, &path.backend, obj).await;

            let Some(dest) = service else {
                continue;
            };

            let path_type = match path.path_type.to_lowercase().as_str() {
                "exact" => ksbh_core::routing::PathType::Exact,
                "prefix" => ksbh_core::routing::PathType::Prefix,
                _ => ksbh_core::routing::PathType::ImplementationSpecific,
            };

            rules_out.push(ksbh_core::routing::IngressRule {
                path: p,
                path_type,
                backend: dest,
            });
        }

        hosts.push(ksbh_core::routing::HostRules {
            host: host.clone(),
            rules: rules_out,
        });
    }

    Ok(hosts)
}

pub(crate) async fn resolve_path_service(
    ctx: &super::IngressController,
    namespace: &str,
    backend: &k8s_openapi::api::networking::v1::IngressBackend,
    obj: &::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
) -> Option<ksbh_core::routing::Backend> {
    if let Some(service) = &backend.service {
        let port = resolve_service_port(ctx, namespace, service).await?;

        let key = (
            ksbh_types::KsbhStr::new(namespace),
            ksbh_types::KsbhStr::new(&service.name),
        );
        ctx.services_refs
            .upsert_sync(key, kube::runtime::reflector::ObjectRef::from_obj(obj));

        let service_name = format!("{}.{}.svc.cluster.local", service.name, namespace);

        Some(ksbh_core::routing::Backend::Upstream {
            name: ksbh_types::KsbhStr::new(service_name),
            port,
        })
    } else if let Some(ressource) = &backend.resource {
        if ressource.api_group.as_ref().is_some_and(|api_group| {
            api_group == ksbh_core::constants::KSBH_K8S_SERVICE_RESSOURCE_API_GROUP
        }) {
            let ressource_kind = ressource.kind.to_lowercase();

            if ressource_kind == ksbh_core::constants::KSBH_SERVICE_RESSOURCE_KIND_STATIC {
                Some(ksbh_core::routing::Backend::Static)
            } else if ressource_kind == "self" {
                tracing::warn!(
                    "Ingress '{}/{}' path resource kind 'self' is deprecated; treating as no backend",
                    namespace,
                    obj.metadata.name.as_deref().unwrap_or("<unknown>")
                );
                Some(ksbh_core::routing::Backend::None)
            } else {
                Some(ksbh_core::routing::Backend::None)
            }
        } else {
            Some(ksbh_core::routing::Backend::None)
        }
    } else {
        Some(ksbh_core::routing::Backend::None)
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
