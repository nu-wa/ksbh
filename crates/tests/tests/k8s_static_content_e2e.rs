mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment with the static ingress and PVC writer job"]
async fn k8s_static_content_is_served_from_shared_pvc() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();

    let response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/",
        "static.example.local",
        reqwest::StatusCode::OK,
    )
    .await;

    let body = response
        .text()
        .await
        .expect("failed to read static content response body");

    assert!(
        body.contains("hello from rwx static content"),
        "expected shared pvc content to be served through ksbh static backend, got `{}`",
        body
    );
}
