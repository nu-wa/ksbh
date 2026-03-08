pub enum ModuleResult {
    Pass,
    Stop(http::Response<bytes::Bytes>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;

    #[test]
    fn test_module_result_pass() {
        let result = ModuleResult::Pass;
        assert!(matches!(result, ModuleResult::Pass));
    }

    #[test]
    fn test_module_result_stop() {
        let response = http::Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(bytes::Bytes::new())
            .unwrap();
        let result = ModuleResult::Stop(response);
        assert!(matches!(result, ModuleResult::Stop(_)));
    }
}
