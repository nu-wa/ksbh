mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment prepared by `mise run e2e`"]
async fn k8s_metrics_endpoint_is_available() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();

    let response = common::wait_for_metrics_ready(&client, &config.metrics_addr).await;
    let body = response
        .text()
        .await
        .expect("failed to read metrics endpoint body");

    assert!(
        body.contains("ksbh_http_requests_total"),
        "expected prometheus output to contain ksbh_http_requests_total"
    );
}

#[tokio::test]
#[ignore = "requires local kind e2e environment prepared by `mise run e2e`"]
async fn k8s_endpoints_are_configured() {
    let config = common::E2eConfig::from_env();

    assert!(config.http_addr.starts_with("http://"));
    assert!(config.https_addr.starts_with("https://"));
    assert!(config.profiling_addr.starts_with("http://"));
    assert!(config.metrics_addr.starts_with("http://"));
    assert!(!config.namespace.is_empty());
}
