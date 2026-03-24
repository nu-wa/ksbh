#[tokio::test]
async fn binary_file_provider_serves_static_backend() {
    let routing_yaml = r#"
ingresses:
  - name: static-ingress
    host: static.test.local
    paths:
      - path: /
        type: prefix
        backend: static
"#;

    let mut fixture = tests::binary::BinaryFixture::new("static", routing_yaml)
        .expect("failed to create binary fixture");

    let host_static_dir = fixture.static_dir().join("static.test.local");
    ::std::fs::create_dir_all(&host_static_dir)
        .expect("failed to create host-scoped static fixture dir");
    ::std::fs::write(
        host_static_dir.join("index.html"),
        "binary static fixture\n",
    )
    .expect("failed to write static fixture file");

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();

    tests::binary::wait_for_host_body(
        &client,
        &fixture.http_base_addr(),
        "/",
        "static.test.local",
        reqwest::StatusCode::OK,
        "binary static fixture\n",
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "static backend did not return expected body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });
}
