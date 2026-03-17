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
    mut shutdown_watch: Option<pingora::server::ShutdownWatch>,
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
) -> Option<::std::net::IpAddr> {
    use ::std::str::FromStr;

    let mut client_addr: Option<::std::net::IpAddr> = session.client_addr();
    let req_headers = &session.headers().headers;

    let forwarded_for_header = req_headers.get("x-forwarded-for");
    let real_ip = req_headers.get("x-real-ip");

    if forwarded_for_header.is_some() || real_ip.is_some() {
        client_addr = match real_ip {
            Some(real_ip) => match real_ip.to_str() {
                Ok(real_ip_str) => ::std::net::IpAddr::from_str(real_ip_str).ok(),
                Err(_) => None,
            },
            None => None,
        };

        if client_addr.is_none() && forwarded_for_header.is_some() {
            client_addr = match forwarded_for_header {
                Some(forwarded_for) => match forwarded_for.to_str() {
                    Ok(forwarded_for_str) => ::std::net::IpAddr::from_str(forwarded_for_str).ok(),
                    Err(_) => None,
                },
                None => None,
            };
        }
    }

    client_addr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_whitespace() {
        let mut s = String::from("hello world");
        remove_whitespace(&mut s);
        assert_eq!(s, "helloworld");
    }

    #[test]
    fn test_remove_whitespace_owned() {
        let s = "hello world";
        let result = remove_whitespace_owned(s);
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_remove_whitespace_multiple_spaces() {
        let mut s = String::from("a   b   c");
        remove_whitespace(&mut s);
        assert_eq!(s, "abc");
    }

    #[test]
    fn test_remove_whitespace_tabs_newlines() {
        let mut s = String::from("a\t\nb");
        remove_whitespace(&mut s);
        assert_eq!(s, "ab");
    }

    #[test]
    fn test_get_filename_with_extension() {
        let path = std::path::Path::new("/path/to/file.txt");
        let result = get_filename(path);
        assert_eq!(result, Some("file".to_string()));
    }

    #[test]
    fn test_get_filename_without_extension() {
        let path = std::path::Path::new("/path/to/file");
        let result = get_filename(path);
        assert_eq!(result, Some("file".to_string()));
    }

    #[test]
    fn test_get_filename_no_stem() {
        let path = std::path::Path::new("/");
        let result = get_filename(path);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_filename_nested() {
        let path = std::path::Path::new("/a/b/c/d.txt");
        let result = get_filename(path);
        assert_eq!(result, Some("d".to_string()));
    }
}
