/// Configure PeerOptions for https upstreams
#[derive(Debug, Clone)]
pub struct PeerOptions {
    pub sni: Option<::std::sync::Arc<str>>,
    pub verify_cert: bool,
    pub altnerative_names: Vec<::std::sync::Arc<str>>,
}
