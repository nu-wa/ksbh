pub(super) async fn reconcile_ingress(
    obj: ::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
    ctx: ::std::sync::Arc<super::IngressController>,
) -> Result<kube::runtime::controller::Action, crate::provider::controller_error::ControllerError> {
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

    let spec = obj.spec.as_ref().ok_or_else(|| {
        crate::provider::controller_error::ControllerError::InvalidIngress(
            "Missing spec for ingress".into(),
        )
    })?;

    if spec
        .ingress_class_name
        .as_ref()
        .is_none_or(|name| name != ksbh_core::constants::INGRESS_CLASS_NAME)
    {
        return Ok(kube::runtime::controller::Action::await_change());
    }

    let rules = spec.rules.clone().ok_or_else(|| {
        crate::provider::controller_error::ControllerError::InvalidIngress(
            "No rules on ingress".into(),
        )
    })?;

    crate::provider::ensure_finalizer(&api, &*obj).await?;

    super::tls::parse_tls_configs(ctx.as_ref(), &namespace, spec.tls.as_ref()).await?;
    super::rules::parse_rules(
        ctx.as_ref(),
        &namespace,
        &name,
        &rules,
        super::annotations::Annotations::new(obj.metadata.annotations.as_ref()),
        &obj,
    )
    .await?;

    Ok(kube::runtime::controller::Action::requeue(
        tokio::time::Duration::from_secs(250),
    ))
}

async fn handle_deletion(
    obj: &k8s_openapi::api::networking::v1::Ingress,
    api: &kube::Api<k8s_openapi::api::networking::v1::Ingress>,
    ctx: &super::IngressController,
    name: &str,
) -> Result<
    Option<kube::runtime::controller::Action>,
    crate::provider::controller_error::ControllerError,
> {
    use kube::Resource;

    if obj.meta().deletion_timestamp.is_some() {
        ctx.hosts.delete_ingress(name);
        crate::provider::remove_finalizer(api, obj).await?;

        return Ok(Some(kube::runtime::controller::Action::await_change()));
    }

    Ok(None)
}
