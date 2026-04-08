#[derive(Debug, Clone)]
pub struct RouterReader {
    pub(crate) router: ::std::sync::Arc<super::Router>,
}

impl RouterReader {
    pub fn find_route(
        &self,
        http_request: &ksbh_types::requests::http_request::HttpRequest,
    ) -> Option<crate::routing::request_match::RequestMatch> {
        self.router.find_route(http_request)
    }

    pub fn get_global_modules_configs(
        &self,
    ) -> Vec<crate::routing::request_match::RequestMatchModule> {
        self.router.get_global_modules()
    }

    pub fn snapshot_runtime_state(&self) -> super::RuntimeStateSnapshot {
        self.router.snapshot_runtime_state()
    }
}
