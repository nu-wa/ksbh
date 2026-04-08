pub struct ModulesController {
    client: kube::Client,
    pub secret_refs: scc::HashMap<
        (ksbh_types::KsbhStr, ksbh_types::KsbhStr),
        kube::runtime::reflector::ObjectRef<ksbh_core::modules::ModuleConfiguration>,
    >,
    pub hosts: ksbh_core::routing::RouterWriter,
}

impl ModulesController {
    pub fn new(client: kube::Client, hosts: ksbh_core::routing::RouterWriter) -> Self {
        Self {
            client,
            secret_refs: scc::HashMap::new(),
            hosts,
        }
    }

    pub fn start(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        use futures::StreamExt;
        let client = self.client.clone();
        let modules_api = kube::Api::<ksbh_core::modules::ModuleConfiguration>::all(client.clone());
        let secrets = self.secret_refs.clone();

        let modules_secret_watcher = move |secret: k8s_openapi::api::core::v1::Secret| {
            use kube::ResourceExt;

            let ns = ksbh_types::KsbhStr::new(secret.namespace().unwrap_or_default());
            let name = ksbh_types::KsbhStr::new(secret.name_any());

            secrets.get_sync(&(ns, name)).map(|e| e.get().clone())
        };

        tokio::spawn(async move {
            let stream = kube::runtime::Controller::new(modules_api, Default::default())
                .watches(
                    kube::Api::<k8s_openapi::api::core::v1::Secret>::all(client),
                    Default::default(),
                    modules_secret_watcher,
                )
                .graceful_shutdown_on(async move {
                    let _ = shutdown.changed().await;
                    tracing::debug!("Stopped modules controller");
                })
                .run(
                    reconcile_modules,
                    error_modules,
                    ::std::sync::Arc::new(self),
                );

            stream
                .for_each(|res| {
                    match res {
                        Ok((obj, _)) => tracing::debug!("Reconcilied module: {}", obj.name),
                        Err(e) => {
                            tracing::error!(
                                "Modules controller stream error: error_type={:?}, error={}",
                                e,
                                e
                            );
                        }
                    }
                    futures::future::ready(())
                })
                .await;
            tracing::warn!("Modules controller task has exited the stream loop");
        })
    }
}

async fn reconcile_modules(
    obj: ::std::sync::Arc<ksbh_core::modules::ModuleConfiguration>,
    ctx: ::std::sync::Arc<ModulesController>,
) -> Result<kube::runtime::controller::Action, super::controller_error::ControllerError> {
    use kube::ResourceExt;

    let name = obj.name_any();
    let api = kube::Api::<ksbh_core::modules::ModuleConfiguration>::all(ctx.client.clone());

    if let Some(action) = handle_module_deletion(&obj, &api, ctx.as_ref(), &name).await? {
        return Ok(action);
    }

    super::ensure_finalizer(&api, &*obj).await?;

    let mut cfg: hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr> =
        hashbrown::HashMap::new();

    if let Some(config) = obj.spec.config.as_ref() {
        for (key, value) in config {
            cfg.insert(
                ksbh_types::KsbhStr::new(key.as_str()),
                ksbh_types::KsbhStr::new(value.as_str()),
            );
        }
    }

    let secret_not_found = fetch_and_update_module_secret(ctx.as_ref(), &obj, &mut cfg).await?;

    if secret_not_found {
        return Ok(kube::runtime::controller::Action::requeue(
            ::std::time::Duration::from_secs(30),
        ));
    }

    ctx.hosts.upsert_module(
        &name,
        obj.spec.global,
        ::std::sync::Arc::new(cfg),
        obj.spec.clone(),
    );

    Ok(kube::runtime::controller::Action::requeue(
        ::std::time::Duration::from_secs(60 * 5),
    ))
}

async fn handle_module_deletion(
    obj: &ksbh_core::modules::ModuleConfiguration,
    api: &kube::Api<ksbh_core::modules::ModuleConfiguration>,
    ctx: &ModulesController,
    name: &str,
) -> Result<Option<kube::runtime::controller::Action>, super::controller_error::ControllerError> {
    use kube::Resource;

    if obj.meta().deletion_timestamp.is_some() {
        if let Some(secret_ref) = &obj.spec.secret_ref {
            let secret_namespace = secret_ref
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string());
            if let Some(secret_name) = secret_ref.name.as_ref() {
                let key = (
                    ksbh_types::KsbhStr::new(secret_namespace.as_str()),
                    ksbh_types::KsbhStr::new(secret_name.as_str()),
                );
                ctx.secret_refs.remove_sync(&key);
            }
        }
        ctx.hosts.delete_module_config(name);
        super::remove_finalizer(api, obj).await?;

        return Ok(Some(kube::runtime::controller::Action::await_change()));
    }

    Ok(None)
}

fn error_modules(
    obj: ::std::sync::Arc<ksbh_core::modules::ModuleConfiguration>,
    err: &super::controller_error::ControllerError,
    _ctx: ::std::sync::Arc<ModulesController>,
) -> kube::runtime::controller::Action {
    tracing::error!("Module: '{:?}', caused an error: '{}", obj, err);
    kube::runtime::controller::Action::requeue(::std::time::Duration::from_secs(30))
}

async fn fetch_and_update_module_secret(
    ctx: &ModulesController,
    obj: &ksbh_core::modules::ModuleConfiguration,
    cfg: &mut hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr>,
) -> Result<bool, super::controller_error::ControllerError> {
    use kube::ResourceExt;

    let Some(secret_ref) = obj.spec.clone().secret_ref else {
        return Ok(false);
    };

    let secret_namespace = secret_ref
        .namespace
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let secret_name = secret_ref.name.clone().unwrap_or_default();

    tracing::debug!(
        "Module '{}' has secret_ref: name='{}', namespace='{}'",
        obj.name_any(),
        secret_name,
        secret_namespace
    );

    let key = (
        ksbh_types::KsbhStr::new(secret_namespace.as_str()),
        ksbh_types::KsbhStr::new(secret_name.as_str()),
    );

    ctx.secret_refs
        .upsert_sync(key, kube::runtime::reflector::ObjectRef::from_obj(obj));

    let Some(secret_name) = secret_ref.name else {
        return Ok(false);
    };

    let secret_api = kube::Api::<k8s_openapi::api::core::v1::Secret>::namespaced(
        ctx.client.clone(),
        &secret_namespace,
    );

    match secret_api.get(&secret_name).await {
        Ok(secret) => {
            let secret_data: Option<Vec<(::std::string::String, ::std::string::String)>> = secret
                .data
                .map(|data| {
                    data.into_iter()
                        .map(|(k, v)| {
                            (k, ::std::string::String::from_utf8_lossy(&v.0).into_owned())
                        })
                        .collect()
                })
                .or_else(|| {
                    secret
                        .string_data
                        .map(|s_data| s_data.into_iter().collect())
                });
            if let Some(ref data) = secret_data {
                for (key, value) in data {
                    cfg.insert(
                        ksbh_types::KsbhStr::new(key.as_str()),
                        ksbh_types::KsbhStr::new(value.as_str()),
                    );
                }
            }
            Ok(false)
        }

        Err(e) => {
            tracing::warn!(
                "Failed to get secret '{}/{}': {}",
                secret_namespace,
                secret_name,
                e
            );
            Ok(true)
        }
    }
}
