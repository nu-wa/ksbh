#[derive(Debug, Clone)]
pub struct RouterWriter {
    pub(super) router: ::std::sync::Arc<super::Router>,
}

impl RouterWriter {
    pub fn delete_module_config(&self, name: &str) {
        self.router.delete_module_config(name);
    }

    pub fn upsert_module(
        &self,
        name: &str,
        global: bool,
        config: crate::modules::ModuleConfigurationValues,
        spec: crate::modules::ModuleConfigurationSpec,
    ) {
        self.router.upsert_module(name, global, config, spec);
    }

    pub fn insert_ingress(
        &self,
        ingress_name: &str,
        hosts: Vec<(::std::sync::Arc<str>, crate::routing::HostPaths)>,
        module_config: crate::routing::IngressModuleConfig,
        peer_options: Option<ksbh_types::providers::proxy::peer_options::PeerOptions>,
    ) {
        self.router
            .insert_ingress(ingress_name, hosts, module_config, peer_options);
    }

    pub fn delete_ingress(&self, ingress_name: &str) {
        self.router.delete_ingress(ingress_name);
    }

    pub fn reload_ingresses(&self) {
        self.router.reload_ingresses();
    }
}
