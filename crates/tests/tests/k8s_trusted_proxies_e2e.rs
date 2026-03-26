mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment with a second trusted-proxy ksbh release"]
async fn k8s_trusted_proxies_gate_forwarded_https_for_http_to_https_module() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let module_name = common::unique_name("trusted-https");
    let ingress_name = common::unique_name("trusted-https-ingress");
    let host = common::unique_host("trusted-https");
    let forwarded_https_headers = [("X-Forwarded-Proto", "https"), ("X-Forwarded-Port", "443")];

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

    let untrusted_response = common::wait_for_host_status_with_headers(
        &client,
        &config.http_addr,
        "/get",
        &host,
        reqwest::StatusCode::MOVED_PERMANENTLY,
        &forwarded_https_headers,
    )
    .await;

    let untrusted_location = untrusted_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("expected redirect location when forwarded headers are untrusted");
    assert_eq!(
        untrusted_location,
        format!("https://{host}/get"),
        "expected untrusted forwarded https headers to be ignored and still redirect",
    );

    let trusted_response = common::wait_for_host_status_with_headers(
        &client,
        &config.trusted_http_addr,
        "/get",
        &host,
        reqwest::StatusCode::OK,
        &forwarded_https_headers,
    )
    .await;

    assert!(
        trusted_response
            .headers()
            .get(reqwest::header::LOCATION)
            .is_none(),
        "expected trusted forwarded https headers to suppress the redirect",
    );

    let body = trusted_response
        .text()
        .await
        .expect("failed to read trusted forwarded https response body");
    assert!(
        body.contains("\"url\"") && body.contains("/get"),
        "expected trusted forwarded https request to reach upstream service body, got `{}`",
        body
    );

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
    common::delete_module_configuration(&kube_client, &module_name).await;
}
