use notify::Watcher;

/// Get an environment variable
///
/// Either from a file (if it ends with `_FILE`), or directly from the environment.
pub fn get_env(key: &str) -> Result<String, Box<dyn ::std::error::Error + 'static>> {
    let env_value = ::std::env::var(key)?;

    if env_value.is_empty() {
        return Err(format!("{key} is empty!").into());
    }

    if key.ends_with("_FILE") {
        return Ok(::std::fs::read_to_string(env_value)?);
    }

    Ok(env_value)
}

// TODO: cleanup or delete this atrocity.
pub fn get_env_prefer_file(key: &str) -> Result<String, Box<dyn ::std::error::Error + 'static>> {
    let key_file = format!(
        "{}{}",
        key,
        match key.ends_with("_FILE") {
            true => "",
            false => "_FILE",
        }
    );

    match get_env(&key_file) {
        Err(_) => get_env(key),
        Ok(v) => Ok(v),
    }
}

pub fn current_unix_time() -> i64 {
    ::std::time::SystemTime::now()
        .duration_since(::std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn remove_whitespace(s: &mut String) {
    s.retain(|c| !c.is_whitespace());
}

pub fn remove_whitespace_owned(s: &str) -> String {
    let mut r = String::from(s);

    remove_whitespace(&mut r);

    r
}

pub fn create_required_directories(configuration: &crate::Config) -> Result<(), ::std::io::Error> {
    ::std::fs::create_dir_all(&configuration.config_paths.config)?;
    ::std::fs::create_dir_all(&configuration.config_paths.modules)?;

    Ok(())
}

/// Helper function to recursively watch a directory.
pub fn watch_directory_files<FnEntry, FnNotify>(
    path: &::std::path::Path,
    mut entry_fn: FnEntry,
    mut notify_fn: FnNotify,
) -> Result<(), notify::Error>
where
    FnEntry: FnMut(&mut walkdir::DirEntry),
    FnNotify: FnMut(&mut notify::Event),
{
    for mut entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        entry_fn(&mut entry);
    }

    let (tx, rx) = ::std::sync::mpsc::channel();

    let mut watcher = notify::recommended_watcher(tx)?;

    watcher.watch(
        ::std::path::Path::new(path),
        notify::RecursiveMode::Recursive,
    )?;

    for mut notify_event in rx.iter().filter_map(|event| event.ok()) {
        notify_fn(&mut notify_event);
    }

    Ok(())
}

pub async fn watch_directory_files_async<S, FnEntry, FnNotify, FutEntry, FutNotify>(
    path: S,
    entry_fn: FnEntry,
    notify_fn: FnNotify,
    mut shutdown_watch: Option<pingora_core::server::ShutdownWatch>,
) -> Result<(), notify::Error>
where
    FnEntry: Fn(walkdir::DirEntry) -> FutEntry + 'static,
    FnNotify: Fn(notify::Event) -> FutNotify + 'static,
    S: AsRef<::std::path::Path>,
    FutEntry: ::std::future::Future<Output = ()> + Send + 'static,
    FutNotify: ::std::future::Future<Output = ()> + Send + 'static,
{
    for entry in walkdir::WalkDir::new(path.as_ref())
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        entry_fn(entry).await;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel(2500);

    let mut watcher = notify::RecommendedWatcher::new(
        move |result: std::result::Result<notify::Event, notify::Error>| {
            if tx.is_closed() {
                return;
            }
            if let Err(e) = tx.blocking_send(result) {
                tracing::debug!("Failed to send event: {}", e);
            }
        },
        notify::Config::default(),
    )?;

    watcher.watch(path.as_ref(), notify::RecursiveMode::Recursive)?;

    loop {
        tokio::select! {
            Some(notify_event) = rx.recv() => {
                match notify_event {
                    Ok(event) => {
                        notify_fn(event).await;
                    }
                    Err(e) => {
                        tracing::error!("Watch error: {e}");
                    }
                }
            }
            _ = async {
                if let Some(shutdown) = &mut shutdown_watch {
                    shutdown.changed().await
                } else {
                    std::future::pending().await
                }
            } => {
                drop(watcher);
                break;
            }

        }
    }

    Ok(())
}

pub fn get_filename(path: &::std::path::Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.to_string())
}

pub fn get_client_ip_from_session(
    session: &dyn ksbh_types::prelude::ProxyProviderSession,
    trust_forwarded_headers: bool,
) -> Option<::std::net::IpAddr> {
    use ::std::str::FromStr;

    fn parse_forwarded_ip(
        value: &http::header::HeaderValue,
        allow_chain: bool,
    ) -> Option<::std::net::IpAddr> {
        let value = value.to_str().ok()?;
        let candidate = if allow_chain {
            value.split(',').next()?.trim()
        } else {
            value.trim()
        };
        ::std::net::IpAddr::from_str(candidate).ok()
    }

    let mut client_addr: Option<::std::net::IpAddr> = session.client_addr();
    let req_headers = &session.headers().headers;

    let forwarded_for_header = req_headers.get("x-forwarded-for");
    let real_ip = req_headers.get("x-real-ip");

    if trust_forwarded_headers && (forwarded_for_header.is_some() || real_ip.is_some()) {
        client_addr = real_ip.and_then(|real_ip| parse_forwarded_ip(real_ip, false));

        if client_addr.is_none() && forwarded_for_header.is_some() {
            client_addr = forwarded_for_header
                .and_then(|forwarded_for| parse_forwarded_ip(forwarded_for, true));
        }
    }

    client_addr
}

#[cfg(test)]
mod tests {
    struct TestSession {
        parts: http::request::Parts,
        client_addr: Option<::std::net::IpAddr>,
    }

    #[async_trait::async_trait]
    impl ksbh_types::prelude::ProxyProviderSession for TestSession {
        fn headers(&self) -> http::request::Parts {
            self.parts.clone()
        }

        fn get_header(&self, header_name: http::HeaderName) -> Option<&http::header::HeaderValue> {
            self.parts.headers.get(header_name)
        }

        fn set_request_uri(&mut self, uri: http::Uri) {
            self.parts.uri = uri;
        }

        fn server_addr(&self) -> Option<::std::net::SocketAddr> {
            None
        }

        fn response_written(&self) -> Option<http::Response<bytes::Bytes>> {
            None
        }

        fn response_sent(&self) -> bool {
            false
        }

        fn client_addr(&self) -> Option<::std::net::IpAddr> {
            self.client_addr
        }

        async fn write_response(
            &mut self,
            _response: http::Response<bytes::Bytes>,
        ) -> Result<(), ksbh_types::prelude::ProxyProviderError> {
            Ok(())
        }

        async fn read_request_body(
            &mut self,
        ) -> Result<Option<bytes::Bytes>, ksbh_types::prelude::ProxyProviderError> {
            Ok(None)
        }
    }

    fn build_session(client_addr: &str, headers: &[(&str, &str)]) -> TestSession {
        let mut builder = http::Request::builder()
            .method(http::Method::GET)
            .uri("http://example.test/");
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let (parts, _) = builder.body(()).expect("build test request").into_parts();

        TestSession {
            parts,
            client_addr: Some(client_addr.parse().expect("parse client ip")),
        }
    }

    #[test]
    fn forwarded_headers_are_ignored_for_untrusted_proxy() {
        let session = build_session(
            "10.0.0.10",
            &[
                ("X-Real-IP", "203.0.113.8"),
                ("X-Forwarded-For", "198.51.100.5"),
            ],
        );

        assert_eq!(
            super::get_client_ip_from_session(&session, false),
            Some("10.0.0.10".parse().expect("parse direct client ip"))
        );
    }

    #[test]
    fn real_ip_takes_precedence_for_trusted_proxy() {
        let session = build_session(
            "10.0.0.10",
            &[
                ("X-Real-IP", "203.0.113.8"),
                ("X-Forwarded-For", "198.51.100.5"),
            ],
        );

        assert_eq!(
            super::get_client_ip_from_session(&session, true),
            Some("203.0.113.8".parse().expect("parse x-real-ip"))
        );
    }

    #[test]
    fn forwarded_for_uses_first_hop_for_trusted_proxy() {
        let session = build_session(
            "10.0.0.10",
            &[("X-Forwarded-For", "198.51.100.5, 10.0.0.10")],
        );

        assert_eq!(
            super::get_client_ip_from_session(&session, true),
            Some(
                "198.51.100.5"
                    .parse()
                    .expect("parse first x-forwarded-for hop")
            )
        );
    }
}
