/// Routing destination — where the proxy forwards a matched request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoutingDestination {
    /// Forward to a named upstream
    Upstream(Upstream),
    /// Serve static content
    Static,
    /// Return an error response with static message
    Error(&'static str),
    /// No destination configured
    None,
}

/// An upstream endpoint — the name and port of a proxied service.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Upstream {
    /// Upstream name identifier
    pub name: ksbh_types::KsbhStr,
    /// Port number
    pub port: u16,
}
