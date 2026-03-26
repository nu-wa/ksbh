mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment with dynamic http_to_https module configuration support"]
async fn k8s_http_to_https_module_redirects_http_and_passes_https() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let module_name = common::unique_name("https");
    let ingress_name = common::unique_name("https-ingress");
    let host = common::unique_host("https");

    common::create_http_to_https_module(&kube_client, &module_name, false).await;

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-httpbin",
        &[module_name.as_str()],
        &[],
    )
    .await;

    let http_response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/get",
        &host,
        reqwest::StatusCode::MOVED_PERMANENTLY,
    )
    .await;

    let location = http_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("expected location header on http_to_https redirect");
    assert_eq!(
        location,
        format!("https://{host}/get"),
        "expected http request to redirect to the https URL for the same host/path",
    );

    let https_response = common::wait_for_host_status(
        &client,
        &config.https_addr,
        "/get",
        &host,
        reqwest::StatusCode::OK,
    )
    .await;

    assert!(
        https_response
            .headers()
            .get(reqwest::header::LOCATION)
            .is_none(),
        "expected direct https request to be served without another redirect",
    );

    let body = https_response
        .text()
        .await
        .expect("failed to read httpbin response body over https");
    assert!(
        body.contains("\"url\"") && body.contains("/get"),
        "expected https request to reach upstream service body, got `{}`",
        body
    );

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
    common::delete_module_configuration(&kube_client, &module_name).await;
}
