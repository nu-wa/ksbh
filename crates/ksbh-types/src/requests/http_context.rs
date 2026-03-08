#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HttpContext {
    pub request: super::HttpRequest,
}
