mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment prepared by `mise run e2e`"]
async fn k8s_ingress_is_added_removed_and_recreated() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let ingress_name = common::unique_name("ingress-lifecycle");
    let host = common::unique_host("ingress-lifecycle");

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-httpbin",
        &[],
        &[],
    )
    .await;

    let response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/",
        &host,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;

    let missing_response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/",
        &host,
        reqwest::StatusCode::NOT_FOUND,
    )
    .await;

    assert_eq!(missing_response.status(), reqwest::StatusCode::NOT_FOUND);

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-httpbin",
        &[],
        &[],
    )
    .await;

    let recreated_response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/",
        &host,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(recreated_response.status(), reqwest::StatusCode::OK);

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
}
