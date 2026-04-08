pub(super) mod controller_error;
pub(super) mod ingress_controller;
pub(super) mod module_controller;

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

        let modules_controller =
            module_controller::ModulesController::new(client.clone(), router.clone());

        let modules_controller_handle =
            module_controller::ModulesController::start(modules_controller, shutdown.clone());

        let ingress_controller =
            ingress_controller::IngressController::new(client.clone(), certs, router);

        let ingress_controller_handle =
            ingress_controller::IngressController::start(ingress_controller, shutdown.clone());

        tracing::debug!("Started kubernetes controllers");

        tokio::select! {
            _ = shutdown.changed() => {
                tracing::debug!("Stopping kubernetes background service");
            }
            res = modules_controller_handle => {
                tracing::debug!("Modules controller stopped: {:?}", res);
            }
            res = ingress_controller_handle => {
                tracing::debug!("Ingress controller stopped: {:?}", res);
            }
        }

        tracing::debug!("Stopped kubernetes background service");
    }
}

pub(super) async fn ensure_finalizer<T>(
    api: &kube::Api<T>,
    obj: &T,
) -> Result<(), controller_error::ControllerError>
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

pub(super) async fn remove_finalizer<T>(
    api: &kube::Api<T>,
    obj: &T,
) -> Result<(), controller_error::ControllerError>
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
