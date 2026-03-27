#![allow(dead_code)]

const DEFAULT_WAIT_RETRIES: usize = 2;
const DEFAULT_WAIT_RETRY_DELAY: tokio::time::Duration = tokio::time::Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
pub struct WaitRetrySettings {
    pub retries: usize,
    pub retry_delay: tokio::time::Duration,
}

impl Default for WaitRetrySettings {
    fn default() -> Self {
        Self {
            retries: DEFAULT_WAIT_RETRIES,
            retry_delay: DEFAULT_WAIT_RETRY_DELAY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct E2eConfig {
    pub http_addr: ::std::string::String,
    pub https_addr: ::std::string::String,
    pub trusted_http_addr: ::std::string::String,
    pub trusted_https_addr: ::std::string::String,
    pub profiling_addr: ::std::string::String,
    pub metrics_addr: ::std::string::String,
    pub namespace: ::std::string::String,
}

impl E2eConfig {
    pub fn from_env() -> Self {
        Self {
            http_addr: ::std::env::var("KSBH_E2E_HTTP_ADDR")
                .unwrap_or_else(|_| "http://127.0.0.1:18080".to_string()),
            https_addr: ::std::env::var("KSBH_E2E_HTTPS_ADDR")
                .unwrap_or_else(|_| "https://127.0.0.1:18443".to_string()),
            trusted_http_addr: ::std::env::var("KSBH_E2E_TRUSTED_HTTP_ADDR")
                .unwrap_or_else(|_| "http://127.0.0.1:19080".to_string()),
            trusted_https_addr: ::std::env::var("KSBH_E2E_TRUSTED_HTTPS_ADDR")
                .unwrap_or_else(|_| "https://127.0.0.1:19443".to_string()),
            profiling_addr: ::std::env::var("KSBH_E2E_PROFILING_ADDR")
                .unwrap_or_else(|_| "http://127.0.0.1:18083".to_string()),
            metrics_addr: ::std::env::var("KSBH_E2E_METRICS_ADDR")
                .unwrap_or_else(|_| "http://127.0.0.1:18084".to_string()),
            namespace: ::std::env::var("KSBH_E2E_NAMESPACE").unwrap_or_else(|_| "ksbh".to_string()),
        }
    }
}

pub struct ModuleConfigurationInput<'a> {
    pub module_name: &'a str,
    pub module_type: &'a str,
    pub weight: i32,
    pub global: bool,
    pub requires_body: bool,
    pub secret_name: Option<&'a str>,
    pub secret_namespace: Option<&'a str>,
}

pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(tokio::time::Duration::from_secs(5))
        .build()
        .expect("failed to create reqwest client for e2e tests")
}

pub async fn kube_client() -> kube::Client {
    kube::Client::try_default()
        .await
        .expect("failed to construct Kubernetes client for e2e tests")
}

pub fn unique_suffix() -> ::std::string::String {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

pub fn unique_name(prefix: &str) -> ::std::string::String {
    format!("{}-{}", prefix, unique_suffix())
}

pub fn unique_host(prefix: &str) -> ::std::string::String {
    format!("{}.e2e.local", unique_name(prefix))
}

pub async fn get_with_host(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    get_with_host_and_headers(client, base_addr, path, host, &[]).await
}

pub async fn get_with_host_and_headers(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    extra_headers: &[(&str, &str)],
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = client
        .get(format!("{base_addr}{path}"))
        .header(reqwest::header::HOST, host);

    for (name, value) in extra_headers {
        request = request.header(*name, *value);
    }

    client
        .execute(
            request
                .build()
                .expect("failed to build request with host headers"),
        )
        .await
}

pub async fn wait_for_host_status_with_headers(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    extra_headers: &[(&str, &str)],
) -> reqwest::Response {
    wait_for_host_status_with_headers_and_retries(
        client,
        base_addr,
        path,
        host,
        expected_status,
        extra_headers,
        WaitRetrySettings::default(),
    )
    .await
}

pub async fn wait_for_host_status_with_headers_and_retries(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    extra_headers: &[(&str, &str)],
    retry_settings: WaitRetrySettings,
) -> reqwest::Response {
    let mut attempts = 0usize;
    let mut last_error = ::std::string::String::new();

    while attempts <= retry_settings.retries {
        attempts += 1;
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(45);

        while start.elapsed() < timeout {
            match get_with_host_and_headers(client, base_addr, path, host, extra_headers).await {
                Ok(response) if response.status() == expected_status => return response,
                Ok(response) => {
                    last_error = format!(
                        "unexpected status {} while waiting for host {} path {}",
                        response.status(),
                        host,
                        path
                    );
                }
                Err(error) => {
                    last_error = format!(
                        "request failed while waiting for host {} path {}: {}",
                        host, path, error
                    );
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if attempts <= retry_settings.retries {
            tokio::time::sleep(retry_settings.retry_delay).await;
        }
    }

    panic!(
        "timed out waiting for host {} path {} to return {}: {}",
        host, path, expected_status, last_error
    );
}

pub async fn wait_for_host_body_with_headers(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    expected_body_contains: &str,
    extra_headers: &[(&str, &str)],
) -> ::std::string::String {
    let mut attempts = 0usize;
    let mut last_error = ::std::string::String::new();

    while attempts <= DEFAULT_WAIT_RETRIES {
        attempts += 1;
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(45);

        while start.elapsed() < timeout {
            match get_with_host_and_headers(client, base_addr, path, host, extra_headers).await {
                Ok(response) => {
                    let status = response.status();
                    let body = response
                        .text()
                        .await
                        .expect("failed to read response body while waiting for host body");

                    if status == expected_status && body.contains(expected_body_contains) {
                        return body;
                    }

                    last_error = format!(
                        "unexpected status/body while waiting for host {} path {}: status={}, body=`{}`",
                        host, path, status, body
                    );
                }
                Err(error) => {
                    last_error = format!(
                        "request failed while waiting for host {} path {} body match: {}",
                        host, path, error
                    );
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if attempts <= DEFAULT_WAIT_RETRIES {
            tokio::time::sleep(DEFAULT_WAIT_RETRY_DELAY).await;
        }
    }

    panic!(
        "timed out waiting for host {} path {} to return {} with expected body fragment: {}",
        host, path, expected_status, last_error
    );
}

pub async fn wait_for_host_status(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
) -> reqwest::Response {
    wait_for_host_status_with_retries(
        client,
        base_addr,
        path,
        host,
        expected_status,
        WaitRetrySettings::default(),
    )
    .await
}

pub async fn wait_for_host_status_with_retries(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    retry_settings: WaitRetrySettings,
) -> reqwest::Response {
    let mut attempts = 0usize;
    let mut last_error = ::std::string::String::new();

    while attempts <= retry_settings.retries {
        attempts += 1;
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(45);

        while start.elapsed() < timeout {
            match get_with_host(client, base_addr, path, host).await {
                Ok(response) if response.status() == expected_status => return response,
                Ok(response) => {
                    last_error = format!(
                        "unexpected status {} while waiting for host {} path {}",
                        response.status(),
                        host,
                        path
                    );
                }
                Err(error) => {
                    last_error = format!(
                        "request failed while waiting for host {} path {}: {}",
                        host, path, error
                    );
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if attempts <= retry_settings.retries {
            tokio::time::sleep(retry_settings.retry_delay).await;
        }
    }

    panic!(
        "timed out waiting for host {} path {} to return {}: {}",
        host, path, expected_status, last_error
    );
}

pub async fn wait_for_host_body(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    expected_body: &str,
) -> ::std::string::String {
    wait_for_host_body_with_retries(
        client,
        base_addr,
        path,
        host,
        expected_status,
        expected_body,
        WaitRetrySettings::default(),
    )
    .await
}

pub async fn wait_for_host_body_with_retries(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    expected_body: &str,
    retry_settings: WaitRetrySettings,
) -> ::std::string::String {
    let mut attempts = 0usize;
    let mut last_error = ::std::string::String::new();

    while attempts <= retry_settings.retries {
        attempts += 1;
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(45);

        while start.elapsed() < timeout {
            match get_with_host(client, base_addr, path, host).await {
                Ok(response) => {
                    let status = response.status();
                    let body = response
                        .text()
                        .await
                        .expect("failed to read response body while waiting for host body");

                    if status == expected_status && body == expected_body {
                        return body;
                    }

                    last_error = format!(
                        "unexpected status/body while waiting for host {} path {}: status={}, body=`{}`",
                        host, path, status, body
                    );
                }
                Err(error) => {
                    last_error = format!(
                        "request failed while waiting for host {} path {} body match: {}",
                        host, path, error
                    );
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if attempts <= retry_settings.retries {
            tokio::time::sleep(retry_settings.retry_delay).await;
        }
    }

    panic!(
        "timed out waiting for host {} path {} to return {} with expected body: {}",
        host, path, expected_status, last_error
    );
}

pub async fn wait_for_metrics_ready(
    client: &reqwest::Client,
    metrics_addr: &str,
) -> reqwest::Response {
    wait_for_metrics_ready_with_retries(client, metrics_addr, WaitRetrySettings::default()).await
}

pub async fn wait_for_metrics_ready_with_retries(
    client: &reqwest::Client,
    metrics_addr: &str,
    retry_settings: WaitRetrySettings,
) -> reqwest::Response {
    let url = format!("{}/metrics", metrics_addr);
    let mut attempts = 0usize;
    let mut last_error = ::std::string::String::new();

    while attempts <= retry_settings.retries {
        attempts += 1;
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(45);

        while start.elapsed() < timeout {
            match client.get(&url).send().await {
                Ok(response) if response.status() == reqwest::StatusCode::OK => return response,
                Ok(response) => {
                    last_error = format!(
                        "unexpected status {} while waiting for {}",
                        response.status(),
                        url
                    );
                }
                Err(error) => {
                    last_error = format!("request to {} failed: {}", url, error);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if attempts <= retry_settings.retries {
            tokio::time::sleep(retry_settings.retry_delay).await;
        }
    }

    panic!(
        "timed out waiting for {} to become ready: {}",
        url, last_error
    );
}

pub async fn fetch_metrics_body(
    client: &reqwest::Client,
    metrics_addr: &str,
) -> ::std::string::String {
    let response = client
        .get(format!("{}/metrics", metrics_addr))
        .send()
        .await
        .expect("failed to fetch metrics endpoint");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::OK,
        "metrics endpoint did not return 200 OK"
    );

    response
        .text()
        .await
        .expect("failed to read metrics response body")
}

pub fn parse_metric_gauge(metrics_body: &str, metric_name: &str) -> Option<i64> {
    for line in metrics_body.lines() {
        if line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };

        if name != metric_name {
            continue;
        }

        let Some(value) = parts.next() else {
            continue;
        };

        let parsed = value.parse::<f64>().ok()?;
        return Some(parsed as i64);
    }

    None
}

pub async fn wait_for_runtime_metric_at_least(
    client: &reqwest::Client,
    metrics_addr: &str,
    metric_name: &str,
    expected_minimum: i64,
) -> i64 {
    wait_for_runtime_metric_at_least_with_retries(
        client,
        metrics_addr,
        metric_name,
        expected_minimum,
        WaitRetrySettings::default(),
    )
    .await
}

pub async fn wait_for_runtime_metric_at_least_with_retries(
    client: &reqwest::Client,
    metrics_addr: &str,
    metric_name: &str,
    expected_minimum: i64,
    retry_settings: WaitRetrySettings,
) -> i64 {
    let mut attempts = 0usize;
    let mut last_error = ::std::string::String::new();

    while attempts <= retry_settings.retries {
        attempts += 1;
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(45);

        while start.elapsed() < timeout {
            let metrics_body = fetch_metrics_body(client, metrics_addr).await;

            if let Some(value) = parse_metric_gauge(&metrics_body, metric_name) {
                if value >= expected_minimum {
                    return value;
                }

                last_error = format!(
                    "metric {} has value {}, expected at least {}",
                    metric_name, value, expected_minimum
                );
            } else {
                last_error = format!("metric {} not found in prometheus output", metric_name);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if attempts <= retry_settings.retries {
            tokio::time::sleep(retry_settings.retry_delay).await;
        }
    }

    panic!(
        "timed out waiting for metric {} to reach at least {}: {}",
        metric_name, expected_minimum, last_error
    );
}

pub async fn create_ingress_for_service(
    client: &kube::Client,
    namespace: &str,
    ingress_name: &str,
    host: &str,
    service_name: &str,
    module_names: &[&str],
    excluded_module_names: &[&str],
) {
    let ingresses = kube::Api::<k8s_openapi::api::networking::v1::Ingress>::namespaced(
        client.clone(),
        namespace,
    );

    let module_annotation = if module_names.is_empty() {
        None
    } else {
        Some(module_names.join(","))
    };

    let mut metadata = serde_json::json!({
        "name": ingress_name,
    });

    if module_annotation.is_some() || !excluded_module_names.is_empty() {
        let mut annotations = serde_json::Map::new();

        if let Some(modules) = module_annotation {
            annotations.insert(
                "ksbh.rs/modules".to_string(),
                serde_json::Value::String(modules),
            );
        }

        if !excluded_module_names.is_empty() {
            annotations.insert(
                "ksbh.rs/excluded-modules".to_string(),
                serde_json::Value::String(excluded_module_names.join(",")),
            );
        }

        metadata["annotations"] = serde_json::Value::Object(annotations);
    }

    let ingress_value = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "Ingress",
        "metadata": metadata,
        "spec": {
            "ingressClassName": "ksbh",
            "rules": [
                {
                    "host": host,
                    "http": {
                        "paths": [
                            {
                                "path": "/",
                                "pathType": "Prefix",
                                "backend": {
                                    "service": {
                                        "name": service_name,
                                        "port": {
                                            "number": 80
                                        }
                                    }
                                }
                            }
                        ]
                    }
                }
            ]
        }
    });

    let ingress: k8s_openapi::api::networking::v1::Ingress =
        serde_json::from_value(ingress_value).expect("failed to build ingress object");

    ingresses
        .create(&kube::api::PostParams::default(), &ingress)
        .await
        .expect("failed to create ingress");
}

pub async fn delete_ingress(client: &kube::Client, namespace: &str, ingress_name: &str) {
    let ingresses = kube::Api::<k8s_openapi::api::networking::v1::Ingress>::namespaced(
        client.clone(),
        namespace,
    );

    match ingresses
        .delete(ingress_name, &kube::api::DeleteParams::default())
        .await
    {
        Ok(_) => {}
        Err(kube::Error::Api(error)) if error.code == 404 => {}
        Err(error) => panic!("failed to delete ingress {}: {}", ingress_name, error),
    }

    wait_for_ingress_deletion(&ingresses, ingress_name).await;
}

async fn wait_for_ingress_deletion(
    ingresses: &kube::Api<k8s_openapi::api::networking::v1::Ingress>,
    ingress_name: &str,
) {
    let timeout = tokio::time::Duration::from_secs(30);
    let start = tokio::time::Instant::now();

    while start.elapsed() < timeout {
        match ingresses.get(ingress_name).await {
            Ok(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Err(kube::Error::Api(error)) if error.code == 404 => {
                return;
            }
            Err(error) => {
                panic!(
                    "failed while waiting for ingress {} deletion: {}",
                    ingress_name, error
                );
            }
        }
    }

    panic!(
        "timed out waiting for ingress {} to be deleted from Kubernetes API",
        ingress_name
    );
}

pub async fn create_secret(
    client: &kube::Client,
    namespace: &str,
    secret_name: &str,
    string_data: serde_json::Value,
) {
    let secrets =
        kube::Api::<k8s_openapi::api::core::v1::Secret>::namespaced(client.clone(), namespace);

    let secret_value = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": secret_name,
            "namespace": namespace,
        },
        "type": "Opaque",
        "stringData": string_data,
    });

    let secret: k8s_openapi::api::core::v1::Secret =
        serde_json::from_value(secret_value).expect("failed to build secret object");

    secrets
        .create(&kube::api::PostParams::default(), &secret)
        .await
        .expect("failed to create secret");
}

pub async fn delete_secret(client: &kube::Client, namespace: &str, secret_name: &str) {
    let secrets =
        kube::Api::<k8s_openapi::api::core::v1::Secret>::namespaced(client.clone(), namespace);

    match secrets
        .delete(secret_name, &kube::api::DeleteParams::default())
        .await
    {
        Ok(_) => {}
        Err(kube::Error::Api(error)) if error.code == 404 => {}
        Err(error) => panic!("failed to delete secret {}: {}", secret_name, error),
    }
}

pub async fn create_module_configuration(
    client: &kube::Client,
    input: ModuleConfigurationInput<'_>,
) {
    let modules = kube::Api::<ksbh_core::modules::ModuleConfiguration>::all(client.clone());

    let secret_ref = input.secret_name.map(|name| {
        serde_json::json!({
            "name": name,
            "namespace": input.secret_namespace.unwrap_or("default"),
        })
    });

    let module_value = serde_json::json!({
        "apiVersion": "modules.ksbh.rs/v1",
        "kind": "ModuleConfiguration",
        "metadata": {
            "name": input.module_name,
        },
        "spec": {
            "name": input.module_name,
            "type": input.module_type,
            "weight": input.weight,
            "global": input.global,
            "requiresBody": input.requires_body,
            "secretRef": secret_ref,
        }
    });

    let module: ksbh_core::modules::ModuleConfiguration =
        serde_json::from_value(module_value).expect("failed to build module configuration");

    modules
        .create(&kube::api::PostParams::default(), &module)
        .await
        .expect("failed to create module configuration");
}

pub async fn delete_module_configuration(client: &kube::Client, module_name: &str) {
    let modules = kube::Api::<ksbh_core::modules::ModuleConfiguration>::all(client.clone());

    match modules
        .delete(module_name, &kube::api::DeleteParams::default())
        .await
    {
        Ok(_) => {}
        Err(kube::Error::Api(error)) if error.code == 404 => {}
        Err(error) => panic!(
            "failed to delete module configuration {}: {}",
            module_name, error
        ),
    }
}

pub async fn create_robots_module(
    client: &kube::Client,
    namespace: &str,
    module_name: &str,
    secret_name: &str,
    content: &str,
) {
    create_secret(
        client,
        namespace,
        secret_name,
        serde_json::json!({
            "content": content,
        }),
    )
    .await;

    create_module_configuration(
        client,
        ModuleConfigurationInput {
            module_name,
            module_type: "RobotsDotTXT",
            weight: 100,
            global: false,
            requires_body: false,
            secret_name: Some(secret_name),
            secret_namespace: Some(namespace),
        },
    )
    .await;
}

pub async fn create_pow_module(
    client: &kube::Client,
    namespace: &str,
    module_name: &str,
    secret_name: &str,
    secret: &str,
    difficulty: usize,
) {
    create_secret(
        client,
        namespace,
        secret_name,
        serde_json::json!({
            "secret": secret,
            "difficulty": difficulty.to_string(),
        }),
    )
    .await;

    create_module_configuration(
        client,
        ModuleConfigurationInput {
            module_name,
            module_type: "POW",
            weight: 100,
            global: false,
            requires_body: true,
            secret_name: Some(secret_name),
            secret_namespace: Some(namespace),
        },
    )
    .await;
}

pub async fn create_http_to_https_module(client: &kube::Client, module_name: &str, global: bool) {
    create_module_configuration(
        client,
        ModuleConfigurationInput {
            module_name,
            module_type: "HttpToHttps",
            weight: 100,
            global,
            requires_body: false,
            secret_name: None,
            secret_namespace: None,
        },
    )
    .await;
}
