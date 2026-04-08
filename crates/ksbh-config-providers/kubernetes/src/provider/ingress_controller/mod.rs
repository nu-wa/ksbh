pub(super) mod annotations;
pub(super) mod reconcile_ingress;
pub(super) mod rules;
pub(super) mod tls;

pub(super) struct IngressController {
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

impl IngressController {
    pub fn new(
        client: kube::Client,
        certs: ksbh_core::certs::CertsWriter,
        hosts: ksbh_core::routing::RouterWriter,
    ) -> Self {
        Self {
            certs,
            hosts,
            client,
            secret_refs: scc::HashMap::new(),
            services_refs: scc::HashMap::new(),
        }
    }

    pub fn start(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let client = self.client.clone();
        let secrets = self.secret_refs.clone();

        let secret_watcher = move |secret: k8s_openapi::api::core::v1::Secret| {
            use kube::ResourceExt;

            let ns = ksbh_types::KsbhStr::new(secret.namespace().unwrap_or_default());
            let name = ksbh_types::KsbhStr::new(secret.name_any());

            secrets.get_sync(&(ns, name)).map(|e| e.get().to_owned())
        };

        let services = self.services_refs.clone();

        let service_watcher = move |service: k8s_openapi::api::core::v1::Service| {
            use kube::ResourceExt;

            let ns = ksbh_types::KsbhStr::new(service.namespace().unwrap_or_default());
            let name = ksbh_types::KsbhStr::new(service.name_any());

            services.get_sync(&(ns, name)).map(|e| e.get().clone())
        };

        let ingress_api =
            kube::Api::<k8s_openapi::api::networking::v1::Ingress>::all(client.clone());

        tokio::spawn(async move {
            use futures::StreamExt;

            let stream = kube::runtime::Controller::new(ingress_api, Default::default())
                .watches(
                    kube::Api::<k8s_openapi::api::core::v1::Secret>::all(client.clone()),
                    Default::default(),
                    secret_watcher,
                )
                .watches(
                    kube::Api::<k8s_openapi::api::core::v1::Service>::all(client.clone()),
                    Default::default(),
                    service_watcher,
                )
                .graceful_shutdown_on(async move {
                    let _ = shutdown.changed().await;
                    tracing::debug!("Stopping ingress controller");
                })
                .run(
                    reconcile_ingress::reconcile_ingress,
                    error_ingress,
                    ::std::sync::Arc::new(self),
                );

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
}

fn error_ingress(
    obj: ::std::sync::Arc<k8s_openapi::api::networking::v1::Ingress>,
    err: &super::controller_error::ControllerError,
    _ctx: ::std::sync::Arc<IngressController>,
) -> kube::runtime::controller::Action {
    tracing::error!("Ingress: '{:?}', caused an error: '{}", obj, err);
    kube::runtime::controller::Action::requeue(::std::time::Duration::from_secs(60 * 5))
}
