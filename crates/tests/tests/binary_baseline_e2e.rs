#[tokio::test]
async fn binary_starts_and_exposes_health_and_metrics() {
    let mut fixture = tests::binary::BinaryFixture::new("baseline", "{}")
        .expect("failed to create binary fixture");
    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();

    tests::binary::wait_for_status(
        &client,
        &fixture.internal_base_addr(),
        "/healthz",
        reqwest::StatusCode::OK,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "internal health check failed: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let metrics_response = tests::binary::wait_for_status(
        &client,
        &fixture.metrics_base_addr(),
        "/metrics",
        reqwest::StatusCode::OK,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "metrics endpoint failed: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let metrics_body = metrics_response
        .text()
        .await
        .expect("failed to read metrics response body");

    assert!(
        metrics_body.contains("ksbh_runtime_active_ingresses"),
        "metrics body missing runtime router metric\nlogs:\n{}",
        fixture.logs()
    );

    tests::binary::wait_for_status(
        &client,
        &fixture.http_base_addr(),
        "/",
        reqwest::StatusCode::NOT_FOUND,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "default route did not return 404: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });
}
