#[derive(Debug, Clone)]
pub(super) struct Ingress {
    pub name: ::std::sync::Arc<str>,
    pub merged_modules: super::request_match::RequestMatchModules,
    pub peer_options: Option<ksbh_types::providers::proxy::peer_options::PeerOptions>,
}
