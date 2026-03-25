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

#[tokio::test]
async fn binary_file_provider_static_head_returns_ok_for_existing_file() {
    let routing_yaml = r#"
ingresses:
  - name: static-ingress
    host: static.test.local
    paths:
      - path: /
        type: prefix
        backend: static
"#;

    let mut fixture = tests::binary::BinaryFixture::new("static-head", routing_yaml)
        .expect("failed to create binary fixture");

    let host_static_dir = fixture.static_dir().join("static.test.local");
    ::std::fs::create_dir_all(&host_static_dir)
        .expect("failed to create host-scoped static fixture dir");
    let body = "apiVersion: v1\n";
    ::std::fs::write(host_static_dir.join("index.yaml"), body)
        .expect("failed to write static fixture file");

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    tests::binary::wait_for_host_body(
        &client,
        &fixture.http_base_addr(),
        "/index.yaml",
        "static.test.local",
        reqwest::StatusCode::OK,
        body,
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "static backend did not return expected body: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let head_response = client
        .request(
            reqwest::Method::HEAD,
            format!("{}/index.yaml", fixture.http_base_addr()),
        )
        .header(reqwest::header::HOST, "static.test.local")
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .unwrap_or_else(|error| panic!("failed to request static HEAD: {error}"));

    assert_eq!(
        head_response.status(),
        reqwest::StatusCode::OK,
        "expected static HEAD response to be 200\nlogs:\n{}",
        fixture.logs()
    );

    let content_length = head_response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            panic!(
                "missing/invalid content-length on static HEAD response\nlogs:\n{}",
                fixture.logs()
            )
        });
    assert_eq!(
        content_length,
        body.len(),
        "unexpected content-length on static HEAD response\nlogs:\n{}",
        fixture.logs()
    );

    let head_body = head_response
        .bytes()
        .await
        .unwrap_or_else(|error| panic!("failed to read static HEAD response body: {error}"));
    assert!(
        head_body.is_empty(),
        "expected empty body for static HEAD response, got {} bytes\nlogs:\n{}",
        head_body.len(),
        fixture.logs()
    );
}

#[tokio::test]
async fn binary_file_provider_static_head_missing_returns_not_found() {
    let routing_yaml = r#"
ingresses:
  - name: static-ingress
    host: static.test.local
    paths:
      - path: /
        type: prefix
        backend: static
"#;

    let mut fixture = tests::binary::BinaryFixture::new("static-head-missing", routing_yaml)
        .expect("failed to create binary fixture");

    let host_static_dir = fixture.static_dir().join("static.test.local");
    ::std::fs::create_dir_all(&host_static_dir)
        .expect("failed to create host-scoped static fixture dir");
    ::std::fs::write(host_static_dir.join("index.html"), "ok\n")
        .expect("failed to write static fixture file");

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    tests::binary::wait_for_host_body(
        &client,
        &fixture.http_base_addr(),
        "/",
        "static.test.local",
        reqwest::StatusCode::OK,
        "ok\n",
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "static backend did not become ready before HEAD missing test: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let head_response = client
        .request(
            reqwest::Method::HEAD,
            format!("{}/does-not-exist", fixture.http_base_addr()),
        )
        .header(reqwest::header::HOST, "static.test.local")
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to request static HEAD: {error}\nlogs:\n{}",
                fixture.logs()
            )
        });

    assert_eq!(
        head_response.status(),
        reqwest::StatusCode::NOT_FOUND,
        "expected static HEAD missing response to be 404\nlogs:\n{}",
        fixture.logs()
    );

    let head_body = head_response
        .bytes()
        .await
        .unwrap_or_else(|error| panic!("failed to read static HEAD response body: {error}"));
    assert!(
        head_body.is_empty(),
        "expected empty body for static HEAD 404 response, got {} bytes\nlogs:\n{}",
        head_body.len(),
        fixture.logs()
    );
}

#[tokio::test]
async fn binary_file_provider_static_options_returns_method_not_allowed() {
    let routing_yaml = r#"
ingresses:
  - name: static-ingress
    host: static.test.local
    paths:
      - path: /
        type: prefix
        backend: static
"#;

    let mut fixture = tests::binary::BinaryFixture::new("static-options", routing_yaml)
        .expect("failed to create binary fixture");

    let host_static_dir = fixture.static_dir().join("static.test.local");
    ::std::fs::create_dir_all(&host_static_dir)
        .expect("failed to create host-scoped static fixture dir");
    ::std::fs::write(host_static_dir.join("index.html"), "ok\n")
        .expect("failed to write static fixture file");

    fixture.start().expect("failed to start ksbh binary");

    let client = tests::binary::build_http_client();
    tests::binary::wait_for_host_body(
        &client,
        &fixture.http_base_addr(),
        "/",
        "static.test.local",
        reqwest::StatusCode::OK,
        "ok\n",
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "static backend did not become ready before OPTIONS test: {error}\nlogs:\n{}",
            fixture.logs()
        )
    });

    let response = client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/", fixture.http_base_addr()),
        )
        .header(reqwest::header::HOST, "static.test.local")
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "failed to request static OPTIONS: {error}\nlogs:\n{}",
                fixture.logs()
            )
        });

    assert_eq!(
        response.status(),
        reqwest::StatusCode::METHOD_NOT_ALLOWED,
        "expected OPTIONS against static backend to be 405\nlogs:\n{}",
        fixture.logs()
    );

    let allow_header = response
        .headers()
        .get(reqwest::header::ALLOW)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("missing/invalid Allow header\nlogs:\n{}", fixture.logs()));
    assert_eq!(
        allow_header,
        "GET, HEAD",
        "unexpected Allow header value\nlogs:\n{}",
        fixture.logs()
    );
}
