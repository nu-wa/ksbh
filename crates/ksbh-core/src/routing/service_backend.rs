#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceBackendType {
    ServiceBackend(ServiceBackend),
    ToSelf(Option<ksbh_types::KsbhStr>),
    Static,
    Error(&'static str),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceBackend {
    pub name: ksbh_types::KsbhStr,
    pub port: u16,
}
