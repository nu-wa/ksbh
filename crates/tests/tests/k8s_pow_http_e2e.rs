mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment with dynamic pow module configuration support"]
async fn k8s_pow_module_returns_a_challenge_page_for_unverified_clients() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let module_name = common::unique_name("pow");
    let secret_name = common::unique_name("pow-secret");
    let ingress_name = common::unique_name("pow-ingress");
    let host = common::unique_host("pow");

    common::create_pow_module(
        &kube_client,
        &config.namespace,
        &module_name,
        &secret_name,
        "fd6b17dbda13c02386460d1419fef345a39e2a08c1fc7f69d894b206cd38a1c1",
        1,
    )
    .await;

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

    let response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/",
        &host,
        reqwest::StatusCode::UNAUTHORIZED,
    )
    .await;

    let body = response
        .text()
        .await
        .expect("failed to read pow challenge page");

    assert!(
        body.contains("powForm") || body.contains("challenge"),
        "expected pow challenge html to contain challenge form markup, got `{}`",
        body
    );

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
    common::delete_module_configuration(&kube_client, &module_name).await;
    common::delete_secret(&kube_client, &config.namespace, &secret_name).await;
}
