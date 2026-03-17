pub mod ingress;

pub struct KubeConfigProvider;

impl KubeConfigProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KubeConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ksbh_core::config_provider::ConfigProvider for KubeConfigProvider {
    async fn start(
        &self,
        router: ksbh_core::routing::RouterWriter,
        certs: ksbh_core::certs::CertsWriter,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let client = match kube::Client::try_default().await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    "Failed to connect to Kubernetes: {}. Kubernetes config provider will not be available.",
                    e
                );
                return;
            }
        };

        let hosts = router;
        let certs_writer = certs;

        let modules_controller_ctx = create_modules_context(client.clone(), hosts.clone());

        let modules_api = kube::Api::<ksbh_core::modules::ModuleConfiguration>::all(client.clone());

        let modules_secret_watcher = create_modules_secret_watcher(modules_controller_ctx.clone());

        let shutdown_modules = shutdown.clone();

        let modules_controller_handle = spawn_modules_controller(
            modules_api,
            client.clone(),
            modules_controller_ctx,
            modules_secret_watcher,
            shutdown_modules,
        );

        let ingress_controller_handle =
            Self::create_ingress_task(client.clone(), hosts, certs_writer, shutdown.clone());

        tracing::debug!("Started kubernetes controllers");

        tokio::select! {
            _ = shutdown.changed() => {
                tracing::info!("Stopping kubernetes background service");
            }
            res = modules_controller_handle => {
                tracing::error!("Modules controller stopped: {:?}", res);
            }
            res = ingress_controller_handle => {
                tracing::error!("Ingress controller stopped: {:?}", res);
            }
        }

        tracing::debug!("Stopped kubernetes background service");
    }
}

fn create_modules_context(
    client: kube::Client,
    hosts: ksbh_core::routing::RouterWriter,
) -> ::std::sync::Arc<ModulesCtx> {
    ::std::sync::Arc::new(ModulesCtx {
        hosts,
        client,
        secret_refs: scc::HashMap::new(),
    })
}

fn create_modules_secret_watcher(
    ctx: ::std::sync::Arc<ModulesCtx>,
) -> impl Fn(
    k8s_openapi::api::core::v1::Secret,
) -> Option<kube::runtime::reflector::ObjectRef<ksbh_core::modules::ModuleConfiguration>> {
    move |secret: k8s_openapi::api::core::v1::Secret| {
        use kube::ResourceExt;

        let ns = ksbh_types::KsbhStr::new(secret.namespace().unwrap_or_default());
        let name = ksbh_types::KsbhStr::new(secret.name_any());

        ctx.secret_refs
            .get_sync(&(ns, name))
            .map(|e| e.get().clone())
    }
}

fn spawn_modules_controller(
    modules_api: kube::Api<ksbh_core::modules::ModuleConfiguration>,
    modules_client: kube::Client,
    modules_controller_ctx: ::std::sync::Arc<ModulesCtx>,
    modules_secret_watcher: impl Fn(
        k8s_openapi::api::core::v1::Secret,
    ) -> Option<
        kube::runtime::reflector::ObjectRef<ksbh_core::modules::ModuleConfiguration>,
    > + Send
    + Sync
    + 'static,
    mut shutdown_modules: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    use futures::StreamExt;

    tokio::spawn(async move {
        let stream = kube::runtime::Controller::new(modules_api, Default::default())
            .watches(
                kube::Api::<k8s_openapi::api::core::v1::Secret>::all(modules_client),
                Default::default(),
                modules_secret_watcher,
            )
            .graceful_shutdown_on(async move {
                let _ = shutdown_modules.changed().await;
                tracing::debug!("Stopped modules controller");
            })
            .run(reconcile_modules, error_modules, modules_controller_ctx);

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

impl KubeConfigProvider {
    pub fn create_ingress_task(
        client: kube::Client,
        hosts: ksbh_core::routing::RouterWriter,
        certs: ksbh_core::certs::CertsWriter,
        shutdown_signal: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let context = create_ingress_context(client.clone(), hosts, certs);

        let (ingresses_secret_watcher, ingresses_services_watcher) =
            create_ingress_watchers(context.clone());

        let ingress_api =
            kube::Api::<k8s_openapi::api::networking::v1::Ingress>::all(client.clone());

        spawn_ingress_controller(
            ingress_api,
            context,
            ingresses_secret_watcher,
            ingresses_services_watcher,
            shutdown_signal,
        )
    }
}

fn create_ingress_context(
    client: kube::Client,
    hosts: ksbh_core::routing::RouterWriter,
    certs: ksbh_core::certs::CertsWriter,
) -> ::std::sync::Arc<ingress::IngressCtx> {
    ::std::sync::Arc::new(ingress::IngressCtx {
        hosts,
        client,
        certs,
        secret_refs: scc::HashMap::new(),
        services_refs: scc::HashMap::new(),
    })
}

#[allow(clippy::type_complexity)]
fn create_ingress_watchers(
    ctx: ::std::sync::Arc<ingress::IngressCtx>,
) -> (
    impl Fn(
        k8s_openapi::api::core::v1::Secret,
    )
        -> Option<kube::runtime::reflector::ObjectRef<k8s_openapi::api::networking::v1::Ingress>>
    + Send
    + Sync
    + 'static,
    impl Fn(
        k8s_openapi::api::core::v1::Service,
    )
        -> Option<kube::runtime::reflector::ObjectRef<k8s_openapi::api::networking::v1::Ingress>>
    + Send
    + Sync
    + 'static,
) {
    let ingresses_ctx_secret_watcher = ctx.clone();
    let ingresses_ctx_services_watcher = ctx.clone();

    let ingresses_secret_watcher = move |secret: k8s_openapi::api::core::v1::Secret| {
        use kube::ResourceExt;

        let ns = ksbh_types::KsbhStr::new(secret.namespace().unwrap_or_default());
        let name = ksbh_types::KsbhStr::new(secret.name_any());

        ingresses_ctx_secret_watcher
            .secret_refs
            .get_sync(&(ns, name))
            .map(|e| e.get().to_owned())
    };

    let ingresses_services_watcher = move |service: k8s_openapi::api::core::v1::Service| {
        use kube::ResourceExt;

        let ns = ksbh_types::KsbhStr::new(service.namespace().unwrap_or_default());
        let name = ksbh_types::KsbhStr::new(service.name_any());

        ingresses_ctx_services_watcher
            .services_refs
            .get_sync(&(ns, name))
            .map(|e| e.get().clone())
    };

    (ingresses_secret_watcher, ingresses_services_watcher)
}

fn spawn_ingress_controller(
    ingress_api: kube::Api<k8s_openapi::api::networking::v1::Ingress>,
    context: ::std::sync::Arc<ingress::IngressCtx>,
    ingresses_secret_watcher: impl Fn(
        k8s_openapi::api::core::v1::Secret,
    ) -> Option<
        kube::runtime::reflector::ObjectRef<k8s_openapi::api::networking::v1::Ingress>,
    > + Send
    + Sync
    + 'static,
    ingresses_services_watcher: impl Fn(
        k8s_openapi::api::core::v1::Service,
    ) -> Option<
        kube::runtime::reflector::ObjectRef<k8s_openapi::api::networking::v1::Ingress>,
    > + Send
    + Sync
    + 'static,
    mut shutdown_signal: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    use futures::StreamExt;

    let client = context.client.clone();

    tokio::spawn(async move {
        let stream = kube::runtime::Controller::new(ingress_api, Default::default())
            .watches(
                kube::Api::<k8s_openapi::api::core::v1::Secret>::all(client.clone()),
                Default::default(),
                ingresses_secret_watcher,
            )
            .watches(
                kube::Api::<k8s_openapi::api::core::v1::Service>::all(client.clone()),
                Default::default(),
                ingresses_services_watcher,
            )
            .graceful_shutdown_on(async move {
                let _ = shutdown_signal.changed().await;
                tracing::debug!("Stopping ingress controller");
            })
            .run(ingress::reconcile_ingress, ingress::error_ingress, context);

        stream
            .for_each(|res| {
                match res {
                    Ok((obj, _)) => tracing::debug!("Reconcilied ingress: {}", obj.name),
                    Err(e) => tracing::error!(
                        "IngressController stream error: error_type={:?}, error={}",
                        e,
                        e
                    ),
                }
                futures::future::ready(())
            })
            .await;
        tracing::warn!("Ingress controller task has exited the stream loop");
    })
}

struct ModulesCtx {
    client: kube::Client,
    secret_refs: scc::HashMap<
        (ksbh_types::KsbhStr, ksbh_types::KsbhStr),
        kube::runtime::reflector::ObjectRef<ksbh_core::modules::ModuleConfiguration>,
    >,
    hosts: ksbh_core::routing::RouterWriter,
}

async fn ensure_finalizer<T>(
    api: &kube::Api<T>,
    obj: &T,
) -> Result<(), crate::ingress::ReconcileError>
where
    T: kube::Resource<DynamicType = ()>
        + kube::ResourceExt
        + ::std::clone::Clone
        + ::std::fmt::Debug
        + serde::de::DeserializeOwned,
{
    let meta = obj.meta();
    let name = obj.name_any();

    let has_finalizer = meta
        .finalizers
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|f| f == ksbh_core::constants::KSBH_FINALIZER);

    if !has_finalizer {
        let patch = serde_json::json!({
            "apiVersion": T::api_version(&()),
            "kind": T::kind(&()),
            "metadata": {
                "name": name,
                "finalizers": [ksbh_core::constants::KSBH_FINALIZER],
            }
        });
        let pp = kube::api::PatchParams::apply("ksbh").force();

        api.patch(&name, &pp, &kube::api::Patch::Apply(&patch))
            .await?;
    }

    Ok(())
}

async fn remove_finalizer<T>(api: &kube::Api<T>, obj: &T) -> Result<(), ingress::ReconcileError>
where
    T: kube::Resource
        + kube::ResourceExt
        + ::std::clone::Clone
        + ::std::fmt::Debug
        + serde::de::DeserializeOwned,
{
    let name = obj.name_any();

    let patch = serde_json::json!({
        "metadata": {
            "finalizers": null
        }
    });

    api.patch(
        &name,
        &kube::api::PatchParams::default(),
        &kube::api::Patch::Merge(&patch),
    )
    .await?;

    Ok(())
}

async fn reconcile_modules(
    obj: ::std::sync::Arc<ksbh_core::modules::ModuleConfiguration>,
    ctx: ::std::sync::Arc<ModulesCtx>,
) -> Result<kube::runtime::controller::Action, ingress::ReconcileError> {
    use kube::ResourceExt;

    let name = obj.name_any();
    let api = kube::Api::<ksbh_core::modules::ModuleConfiguration>::all(ctx.client.clone());

    if let Some(action) = handle_module_deletion(&obj, &api, ctx.as_ref(), &name).await? {
        return Ok(action);
    }

    ensure_finalizer(&api, &*obj).await?;

    let mut cfg: hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr> =
        hashbrown::HashMap::new();

    tracing::debug!("Reconciling module: {}", name);

    let secret_not_found = fetch_and_update_module_secret(ctx.as_ref(), &obj, &mut cfg).await?;

    if secret_not_found {
        return Ok(kube::runtime::controller::Action::requeue(
            ::std::time::Duration::from_secs(30),
        ));
    }

    upsert_module_config(ctx.as_ref(), &name, &obj, cfg);

    Ok(kube::runtime::controller::Action::requeue(
        ::std::time::Duration::from_secs(60 * 5),
    ))
}

async fn handle_module_deletion(
    obj: &ksbh_core::modules::ModuleConfiguration,
    api: &kube::Api<ksbh_core::modules::ModuleConfiguration>,
    ctx: &ModulesCtx,
    name: &str,
) -> Result<Option<kube::runtime::controller::Action>, crate::ingress::ReconcileError> {
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
        remove_finalizer(api, obj).await?;

        return Ok(Some(kube::runtime::controller::Action::await_change()));
    }

    Ok(None)
}

async fn fetch_and_update_module_secret(
    ctx: &ModulesCtx,
    obj: &ksbh_core::modules::ModuleConfiguration,
    cfg: &mut hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr>,
) -> Result<bool, crate::ingress::ReconcileError> {
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

    tracing::debug!("Fetching secret '{}/{}'", secret_namespace, secret_name);

    match secret_api.get(&secret_name).await {
        Ok(secret) => {
            tracing::debug!("Found secret '{}/{}'", secret_namespace, secret_name);
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
                tracing::debug!(
                    "Secret '{}' has {} keys: {:?}",
                    secret_name,
                    data.len(),
                    data.iter().map(|(k, _)| k).collect::<Vec<_>>()
                );
                for (k, v) in data.iter() {
                    cfg.insert(
                        ksbh_types::KsbhStr::new(k.as_str()),
                        ksbh_types::KsbhStr::new(v.as_str()),
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

fn upsert_module_config(
    ctx: &ModulesCtx,
    name: &str,
    obj: &ksbh_core::modules::ModuleConfiguration,
    cfg: hashbrown::HashMap<ksbh_types::KsbhStr, ksbh_types::KsbhStr>,
) {
    ctx.hosts.upsert_module(
        name,
        obj.spec.global,
        ::std::sync::Arc::new(cfg),
        obj.spec.clone(),
    );
}

fn error_modules(
    obj: ::std::sync::Arc<ksbh_core::modules::ModuleConfiguration>,
    err: &ingress::ReconcileError,
    _ctx: ::std::sync::Arc<ModulesCtx>,
) -> kube::runtime::controller::Action {
    tracing::error!("Module: '{:?}', caused an error: '{}", obj, err);
    kube::runtime::controller::Action::requeue(::std::time::Duration::from_secs(30))
}

