#[derive(Debug)]
pub struct BinaryFixture {
    config_path: ::std::path::PathBuf,
    modules_dir: ::std::path::PathBuf,
    static_dir: ::std::path::PathBuf,
    tls_cert_path: ::std::path::PathBuf,
    tls_key_path: ::std::path::PathBuf,
    stdout_path: ::std::path::PathBuf,
    stderr_path: ::std::path::PathBuf,
    http_addr: ::std::string::String,
    https_addr: ::std::string::String,
    internal_addr: ::std::string::String,
    metrics_addr: ::std::string::String,
    profiling_addr: ::std::string::String,
    child: Option<::std::process::Child>,
    _temp_dir: tempfile::TempDir,
}

impl BinaryFixture {
    pub fn new(
        name: &str,
        routing_yaml: &str,
    ) -> Result<Self, Box<dyn ::std::error::Error + Send + Sync>> {
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("ksbh-binary-{name}-"))
            .tempdir()?;

        let config_path = temp_dir.path().join("routing.yaml");
        let modules_dir = temp_dir.path().join("modules");
        let static_dir = temp_dir.path().join("static");
        let tls_cert_path = temp_dir.path().join("default-tls.crt");
        let tls_key_path = temp_dir.path().join("default-tls.key");
        let stdout_path = temp_dir.path().join("stdout.log");
        let stderr_path = temp_dir.path().join("stderr.log");

        ::std::fs::create_dir_all(&modules_dir)?;
        ::std::fs::create_dir_all(&static_dir)?;
        ::std::fs::write(&config_path, routing_yaml)?;
        write_default_tls_files(&tls_cert_path, &tls_key_path)?;

        let http_port = find_free_port()?;
        let https_port = find_free_port()?;
        let internal_port = find_free_port()?;
        let metrics_port = find_free_port()?;
        let profiling_port = find_free_port()?;

        Ok(Self {
            config_path,
            modules_dir,
            static_dir,
            tls_cert_path,
            tls_key_path,
            stdout_path,
            stderr_path,
            http_addr: format!("127.0.0.1:{http_port}"),
            https_addr: format!("127.0.0.1:{https_port}"),
            internal_addr: format!("127.0.0.1:{internal_port}"),
            metrics_addr: format!("127.0.0.1:{metrics_port}"),
            profiling_addr: format!("127.0.0.1:{profiling_port}"),
            child: None,
            _temp_dir: temp_dir,
        })
    }

    pub fn start(&mut self) -> Result<(), Box<dyn ::std::error::Error + Send + Sync>> {
        if self.child.is_some() {
            return Ok(());
        }

        let stdout = ::std::fs::File::create(&self.stdout_path)?;
        let stderr = ::std::fs::File::create(&self.stderr_path)?;

        let child = ::std::process::Command::new(binary_path())
            .current_dir(repo_root())
            .env("KSBH__CONFIG_PATHS__CONFIG", &self.config_path)
            .env("KSBH__CONFIG_PATHS__MODULES", &self.modules_dir)
            .env("KSBH__CONFIG_PATHS__STATIC_CONTENT", &self.static_dir)
            .env("KSBH__COOKIE_KEY", default_cookie_key())
            .env("KSBH__CONSTANTS__COOKIE_SECURE", "false")
            .env("KSBH__LISTEN_ADDRESSES__HTTP", &self.http_addr)
            .env("KSBH__LISTEN_ADDRESSES__HTTPS", &self.https_addr)
            .env("KSBH__LISTEN_ADDRESSES__INTERNAL", &self.internal_addr)
            .env("KSBH__LISTEN_ADDRESSES__PROMETHEUS", &self.metrics_addr)
            .env("KSBH__LISTEN_ADDRESSES__PROFILING", &self.profiling_addr)
            .env("KSBH__TLS__DEFAULT_CERT_FILE", &self.tls_cert_path)
            .env("KSBH__TLS__DEFAULT_KEY_FILE", &self.tls_key_path)
            .stdout(::std::process::Stdio::from(stdout))
            .stderr(::std::process::Stdio::from(stderr))
            .spawn()?;

        self.child = Some(child);
        Ok(())
    }

    pub fn static_dir(&self) -> &::std::path::Path {
        &self.static_dir
    }

    pub fn modules_dir(&self) -> &::std::path::Path {
        &self.modules_dir
    }

    pub fn http_base_addr(&self) -> ::std::string::String {
        format!("http://{}", self.http_addr)
    }

    pub fn https_base_addr(&self) -> ::std::string::String {
        format!("https://{}", self.https_addr)
    }

    pub fn internal_base_addr(&self) -> ::std::string::String {
        format!("http://{}", self.internal_addr)
    }

    pub fn metrics_base_addr(&self) -> ::std::string::String {
        format!("http://{}", self.metrics_addr)
    }

    pub fn logs(&self) -> ::std::string::String {
        let stdout = ::std::fs::read_to_string(&self.stdout_path).unwrap_or_default();
        let stderr = ::std::fs::read_to_string(&self.stderr_path).unwrap_or_default();

        format!("stdout:\n{stdout}\n\nstderr:\n{stderr}")
    }
}

impl Drop for BinaryFixture {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(tokio::time::Duration::from_secs(5))
        .build()
        .expect("failed to create reqwest client for binary integration tests")
}

pub async fn get(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    client.get(format!("{base_addr}{path}")).send().await
}

pub async fn get_with_host(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .get(format!("{base_addr}{path}"))
        .header(reqwest::header::HOST, host)
        .send()
        .await
}

pub async fn wait_for_status(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    expected_status: reqwest::StatusCode,
) -> Result<reqwest::Response, ::std::string::String> {
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_secs(20);
    let mut last_error = ::std::string::String::new();

    while start.elapsed() < timeout {
        match get(client, base_addr, path).await {
            Ok(response) if response.status() == expected_status => return Ok(response),
            Ok(response) => {
                last_error = format!(
                    "unexpected status {} while waiting for {}{}",
                    response.status(),
                    base_addr,
                    path
                );
            }
            Err(error) => {
                last_error = format!(
                    "request failed while waiting for {}{}: {}",
                    base_addr, path, error
                );
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }

    Err(last_error)
}

pub async fn wait_for_host_body(
    client: &reqwest::Client,
    base_addr: &str,
    path: &str,
    host: &str,
    expected_status: reqwest::StatusCode,
    expected_body: &str,
) -> Result<::std::string::String, ::std::string::String> {
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::Duration::from_secs(20);
    let mut last_error = ::std::string::String::new();

    while start.elapsed() < timeout {
        match get_with_host(client, base_addr, path, host).await {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.map_err(|error| {
                    format!(
                        "failed to read response body while waiting for host {} path {}: {}",
                        host, path, error
                    )
                })?;

                if status == expected_status && body == expected_body {
                    return Ok(body);
                }

                last_error = format!(
                    "unexpected status/body while waiting for host {} path {}: status={}, body=`{}`",
                    host, path, status, body
                );
            }
            Err(error) => {
                last_error = format!(
                    "request failed while waiting for host {} path {}: {}",
                    host, path, error
                );
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }

    Err(last_error)
}

fn find_free_port() -> Result<u16, Box<dyn ::std::error::Error + Send + Sync>> {
    let listener = ::std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn write_default_tls_files(
    cert_path: &::std::path::Path,
    key_path: &::std::path::Path,
) -> Result<(), Box<dyn ::std::error::Error + Send + Sync>> {
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();
    ::std::fs::write(cert_path, cert_pem)?;
    ::std::fs::write(key_path, key_pem)?;
    Ok(())
}

fn binary_path() -> ::std::path::PathBuf {
    if let Ok(path) = ::std::env::var("KSBH_E2E_BIN") {
        return ::std::path::PathBuf::from(path);
    }

    repo_root()
        .join("crates")
        .join("target")
        .join("debug")
        .join("ksbh")
}

fn repo_root() -> ::std::path::PathBuf {
    let manifest_dir = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(::std::path::Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

fn default_cookie_key() -> &'static str {
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
}
