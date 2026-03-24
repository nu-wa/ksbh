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

#[tokio::test]
async fn binary_file_provider_static_revalidation_returns_not_modified() {
    let routing_yaml = r#"
ingresses:
  - name: static-ingress
    host: static.test.local
    paths:
      - path: /
        type: prefix
        backend: static
"#;

    let mut fixture = tests::binary::BinaryFixture::new("static-etag", routing_yaml)
        .expect("failed to create binary fixture");

    let host_static_dir = fixture.static_dir().join("static.test.local");
    ::std::fs::create_dir_all(&host_static_dir)
        .expect("failed to create host-scoped static fixture dir");
    ::std::fs::write(
        host_static_dir.join("index.html"),
        "binary static fixture etag\n",
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
        "binary static fixture etag\n",
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "static backend did not return expected body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let first_response = client
        .get(format!("{}/", fixture.http_base_addr()))
        .header(reqwest::header::HOST, "static.test.local")
        .send()
        .await
        .unwrap_or_else(|error| panic!("failed to request static content: {error}"));

    assert_eq!(
        first_response.status(),
        reqwest::StatusCode::OK,
        "expected initial static response to be 200\nlogs:\n{}",
        fixture.logs()
    );

    let etag = first_response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(::std::string::ToString::to_string)
        .unwrap_or_else(|| panic!("missing/invalid ETag header on initial static response"));

    let second_response = client
        .get(format!("{}/", fixture.http_base_addr()))
        .header(reqwest::header::HOST, "static.test.local")
        .header(reqwest::header::IF_NONE_MATCH, etag)
        .send()
        .await
        .unwrap_or_else(|error| panic!("failed to request static revalidation: {error}"));

    let second_status = second_response.status();
    let second_body = second_response
        .text()
        .await
        .unwrap_or_else(|error| panic!("failed to read static revalidation body: {error}"));

    assert_eq!(
        second_status,
        reqwest::StatusCode::NOT_MODIFIED,
        "expected static revalidation response to be 304, got {}\nbody=`{}`\nlogs:\n{}",
        second_status,
        second_body,
        fixture.logs()
    );
    assert!(
        second_body.is_empty(),
        "expected empty response body for 304, got `{}`\nlogs:\n{}",
        second_body,
        fixture.logs()
    );
}
