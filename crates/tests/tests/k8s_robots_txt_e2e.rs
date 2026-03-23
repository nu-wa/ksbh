mod common;

#[tokio::test]
#[ignore = "requires local kind e2e environment with dynamic module configuration support"]
async fn k8s_robots_txt_is_served_when_module_is_attached_to_ingress() {
    let config = common::E2eConfig::from_env();
    let client = common::build_http_client();
    let kube_client = common::kube_client().await;
    let fast_retry_settings = common::WaitRetrySettings {
        retries: 0,
        retry_delay: tokio::time::Duration::from_secs(1),
    };
    let module_name = common::unique_name("robots");
    let secret_name = common::unique_name("robots-secret");
    let ingress_name = common::unique_name("robots-ingress");
    let host = common::unique_host("robots");
    let robots_content = "User-agent: *\nDisallow: /\n";
    let baseline_metrics = common::fetch_metrics_body(&client, &config.metrics_addr).await;
    let baseline_ingresses =
        common::parse_metric_gauge(&baseline_metrics, "ksbh_runtime_active_ingresses").unwrap_or(0);
    let baseline_non_global_modules =
        common::parse_metric_gauge(&baseline_metrics, "ksbh_runtime_active_non_global_modules")
            .unwrap_or(0);

    common::create_robots_module(
        &kube_client,
        &config.namespace,
        &module_name,
        &secret_name,
        robots_content,
    )
    .await;

    common::create_ingress_for_service(
        &kube_client,
        &config.namespace,
        &ingress_name,
        &host,
        "e2e-httpbin",
        &[module_name.as_str()],
        &["robots-txt"],
    )
    .await;

    common::wait_for_runtime_metric_at_least_with_retries(
        &client,
        &config.metrics_addr,
        "ksbh_runtime_active_non_global_modules",
        baseline_non_global_modules + 1,
        fast_retry_settings,
    )
    .await;

    common::wait_for_runtime_metric_at_least_with_retries(
        &client,
        &config.metrics_addr,
        "ksbh_runtime_active_ingresses",
        baseline_ingresses + 1,
        fast_retry_settings,
    )
    .await;

    let body = common::wait_for_host_body_with_retries(
        &client,
        &config.http_addr,
        "/robots.txt",
        &host,
        reqwest::StatusCode::OK,
        robots_content,
        fast_retry_settings,
    )
    .await;
    assert_eq!(body, robots_content);

    let app_response = common::wait_for_host_status(
        &client,
        &config.http_addr,
        "/",
        &host,
        reqwest::StatusCode::OK,
    )
    .await;

    assert_eq!(app_response.status(), reqwest::StatusCode::OK);

    common::delete_ingress(&kube_client, &config.namespace, &ingress_name).await;
    common::delete_module_configuration(&kube_client, &module_name).await;
    common::delete_secret(&kube_client, &config.namespace, &secret_name).await;
}
