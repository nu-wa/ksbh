mod file_cache;

#[derive(PartialEq, Eq)]
enum Compression {
    Gzip,
    Zlib,
    Deflate,
    Brotli,
    None,
}

pub struct StaticApp {
    pub(crate) config: ::std::sync::Arc<ksbh_core::Config>,
    file_cache: file_cache::FileCache,
}

struct SessionWriter<'a> {
    session: &'a mut pingora::protocols::http::ServerSession,
}

#[::async_trait::async_trait]
impl<'a> tokio::io::AsyncWrite for SessionWriter<'a> {
    fn poll_write(
        mut self: ::std::pin::Pin<&mut Self>,
        cx: &mut ::std::task::Context<'_>,
        buf: &[u8],
    ) -> ::std::task::Poll<::std::io::Result<usize>> {
        use futures::FutureExt;

        let fut = self
            .session
            .write_response_body(bytes::Bytes::copy_from_slice(buf), false);

        match ::std::pin::Pin::new(&mut fut.boxed()).poll(cx) {
            ::std::task::Poll::Pending => ::std::task::Poll::Pending,
            ::std::task::Poll::Ready(Ok(())) => ::std::task::Poll::Ready(Ok(buf.len())),
            ::std::task::Poll::Ready(Err(e)) => {
                ::std::task::Poll::Ready(Err(::std::io::Error::other(e.to_string())))
            }
        }
    }

    fn poll_flush(
        self: ::std::pin::Pin<&mut Self>,
        _cx: &mut ::std::task::Context<'_>,
    ) -> ::std::task::Poll<::std::io::Result<()>> {
        ::std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: ::std::pin::Pin<&mut Self>,
        _cx: &mut ::std::task::Context<'_>,
    ) -> ::std::task::Poll<::std::io::Result<()>> {
        ::std::task::Poll::Ready(Ok(()))
    }
}

impl StaticApp {
    pub fn new(config: ::std::sync::Arc<ksbh_core::Config>) -> Self {
        Self {
            config,
            file_cache: file_cache::FileCache::new(),
        }
    }

    pub async fn render_static_file(
        &self,
        mut session: pingora::protocols::http::ServerSession,
        _shutdown: &pingora::server::ShutdownWatch,
        host: &str,
        request_path_param: Option<&str>,
        file_param: Option<&str>,
        head_only: bool,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        let file_path = if let Some(file_param) = file_param {
            let decoded = match urlencoding::decode(file_param) {
                Ok(value) => value,
                Err(_) => {
                    return write_404(session, head_only).await;
                }
            };

            match get_clean_file_path(&self.config.config_paths.static_content, &decoded) {
                Some(path) => path,
                None => {
                    return write_404(session, head_only).await;
                }
            }
        } else {
            match resolve_static_file_path(
                &self.config.config_paths.static_content,
                host,
                request_path_param,
            ) {
                Some(path) => path,
                None => {
                    return write_404(session, head_only).await;
                }
            }
        };

        let file_meta = match self.file_cache.get(&file_path).await {
            Some(meta) => meta,
            None => return write_404(session, head_only).await,
        };

        if let Some(if_none) = session.get_header("if-none-match")
            && if_none.as_bytes() == file_meta.etag.as_bytes()
        {
            let mut response_header = pingora::http::ResponseHeader::build(
                pingora::http::StatusCode::NOT_MODIFIED,
                Some(1),
            )
            .ok()?;

            response_header
                .insert_header(http::header::ETAG, file_meta.etag.as_str())
                .ok()?;

            session
                .write_response_header(Box::new(response_header))
                .await
                .ok()?;

            session
                .write_response_body(bytes::Bytes::new(), true)
                .await
                .ok()?;
            return None;
        }

        let mut start = 0;
        let mut end = file_meta.length;
        if let Some(range_header) = session.get_header("range")
            && let Ok(s) = range_header.to_str()
            && s.starts_with("bytes=")
        {
            let parts: ::std::vec::Vec<_> = s["bytes=".len()..].split('-').collect();
            if parts.len() == 2
                && let (Ok(s), Ok(e)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>())
                && s < file_meta.length
                && e < file_meta.length
                && s <= e
            {
                start = s;
                end = e + 1;
            }
        }

        let status = if start == 0 && end == file_meta.length {
            http::StatusCode::OK
        } else {
            http::StatusCode::PARTIAL_CONTENT
        };

        let content_len = end - start;
        let last_mod_val = httpdate::fmt_http_date(file_meta.modified);

        let accept_enc = session
            .get_header("accept-encoding")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let compression = if accept_enc.contains("br") {
            Compression::Brotli
        } else if accept_enc.contains("gzip") {
            Compression::Gzip
        } else if accept_enc.contains("deflate") {
            Compression::Deflate
        } else if accept_enc.contains("zlib") {
            Compression::Zlib
        } else {
            Compression::None
        };
        let use_compression = compression != Compression::None;

        let mut response_header = pingora::http::ResponseHeader::build(status, Some(1)).ok()?;

        response_header
            .insert_header(http::header::ACCEPT_RANGES, "bytes")
            .ok()?;
        response_header
            .insert_header(http::header::CONTENT_TYPE, file_meta.mime.as_str())
            .ok()?;
        response_header
            .insert_header(http::header::ETAG, file_meta.etag.as_str())
            .ok()?;
        response_header
            .insert_header(http::header::LAST_MODIFIED, last_mod_val)
            .ok()?;

        if use_compression {
            response_header
                .insert_header(
                    http::header::CONTENT_ENCODING,
                    match compression {
                        Compression::Gzip => "gzip",
                        Compression::Brotli => "br",
                        Compression::Deflate => "deflate",
                        Compression::Zlib => "zlib",
                        Compression::None => return None,
                    },
                )
                .ok()?;
        } else {
            response_header
                .insert_header(http::header::CONTENT_LENGTH, content_len)
                .ok()?;
        }

        session
            .write_response_header(Box::new(response_header))
            .await
            .ok()?;

        if head_only {
            session
                .write_response_body(bytes::Bytes::new(), true)
                .await
                .ok()?;
            return None;
        }

        let mut offset = start;

        use tokio::io::AsyncWriteExt;
        if use_compression {
            let writer = SessionWriter {
                session: &mut session,
            };

            let mut encoder: Box<dyn tokio::io::AsyncWrite + Unpin + Send> = match compression {
                Compression::Gzip => {
                    Box::new(async_compression::tokio::write::GzipEncoder::new(writer))
                }
                Compression::Zlib => {
                    Box::new(async_compression::tokio::write::ZlibEncoder::new(writer))
                }
                Compression::Deflate => {
                    Box::new(async_compression::tokio::write::DeflateEncoder::new(writer))
                }
                Compression::Brotli => {
                    Box::new(async_compression::tokio::write::BrotliEncoder::new(writer))
                }
                Compression::None => return None,
            };

            while offset < end {
                let chunk_end = ::std::cmp::min(offset + 256 * 1024, end);
                encoder
                    .write_all(&file_meta.mmap[offset..chunk_end])
                    .await
                    .ok()?;
                offset = chunk_end;
            }
            encoder.shutdown().await.ok()?;
        } else {
            while offset < end {
                let chunk_end = ::std::cmp::min(offset + 256 * 1024, end);
                session
                    .write_response_body(
                        bytes::Bytes::copy_from_slice(&file_meta.mmap[offset..chunk_end]),
                        false,
                    )
                    .await
                    .ok()?;
                offset = chunk_end;
            }
            session
                .write_response_body(bytes::Bytes::new(), true)
                .await
                .ok()?;
        }

        None
    }
}

fn get_clean_file_path(root: &::std::path::Path, req_path: &str) -> Option<::std::path::PathBuf> {
    if req_path.contains("..") {
        return None;
    };

    let p = root.join(req_path.trim_start_matches("/"));

    if p.is_file() {
        return Some(p);
    }

    None
}

fn resolve_static_file_path(
    root: &::std::path::Path,
    host: &str,
    request_path: Option<&str>,
) -> Option<::std::path::PathBuf> {
    let host = host.trim_end_matches('/');
    if host.is_empty() {
        return None;
    }

    let raw_path = request_path.unwrap_or("/");
    let decoded_path = urlencoding::decode(raw_path).ok()?.into_owned();
    let normalized_path = if decoded_path.is_empty() {
        "/"
    } else {
        decoded_path.as_str()
    };
    let trimmed_path = normalized_path.trim_start_matches('/');

    let mut candidates = ::std::vec::Vec::new();
    if trimmed_path.is_empty() {
        candidates.push(format!("{host}/index.html"));
    } else if normalized_path.ends_with('/') {
        let dir = trimmed_path.trim_end_matches('/');
        if dir.is_empty() {
            candidates.push(format!("{host}/index.html"));
        } else {
            candidates.push(format!("{host}/{dir}/index.html"));
        }
    } else {
        candidates.push(format!("{host}/{trimmed_path}"));
        candidates.push(format!("{host}/{trimmed_path}/index.html"));
    }

    for candidate in candidates {
        if let Some(path) = get_clean_file_path(root, &candidate) {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    fn make_temp_root() -> ::std::path::PathBuf {
        let root =
            ::std::env::temp_dir().join(format!("ksbh-static-tests-{}", uuid::Uuid::new_v4()));
        ::std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn resolve_static_file_path_maps_root_to_host_index() {
        let root = make_temp_root();
        let host_dir = root.join("ksbh.rs");
        ::std::fs::create_dir_all(&host_dir).expect("create host dir");
        ::std::fs::write(host_dir.join("index.html"), "ok").expect("write index");

        let resolved = super::resolve_static_file_path(&root, "ksbh.rs", Some("/"))
            .expect("resolve root index");
        assert_eq!(resolved, host_dir.join("index.html"));

        ::std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn resolve_static_file_path_prefers_file_for_non_trailing_path() {
        let root = make_temp_root();
        let host_dir = root.join("ksbh.rs");
        ::std::fs::create_dir_all(&host_dir).expect("create host dir");
        ::std::fs::write(host_dir.join("xd"), "ok").expect("write file");

        let resolved = super::resolve_static_file_path(&root, "ksbh.rs", Some("/xd"))
            .expect("resolve direct file");
        assert_eq!(resolved, host_dir.join("xd"));

        ::std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn resolve_static_file_path_falls_back_to_directory_index() {
        let root = make_temp_root();
        let host_dir = root.join("ksbh.rs");
        let docs_dir = host_dir.join("docs");
        ::std::fs::create_dir_all(&docs_dir).expect("create docs dir");
        ::std::fs::write(docs_dir.join("index.html"), "ok").expect("write docs index");

        let resolved_without_slash =
            super::resolve_static_file_path(&root, "ksbh.rs", Some("/docs"))
                .expect("resolve docs index without trailing slash");
        assert_eq!(resolved_without_slash, docs_dir.join("index.html"));

        let resolved_with_slash = super::resolve_static_file_path(&root, "ksbh.rs", Some("/docs/"))
            .expect("resolve docs index with trailing slash");
        assert_eq!(resolved_with_slash, docs_dir.join("index.html"));

        ::std::fs::remove_dir_all(&root).expect("cleanup");
    }
}

/// Write a 404 response on `session` and return `None` to the caller.
///
/// The static app signals "I could not serve this path" by writing a 404
/// response on the session and returning `None`. Returning `None` from
/// `HttpServerApp::process_new_http` without writing a response is treated by
/// pingora as "I did not handle this" and the request falls through to the
/// upstream proxy, which 502s for backends that are not listening. Always
/// writing a response keeps the seam at the static app's interface honest.
async fn write_404(
    mut session: pingora::protocols::http::ServerSession,
    head_only: bool,
) -> Option<pingora::apps::ReusedHttpStream> {
    let page = match ksbh_ui::error_pages::render_error_page_html("404") {
        Some(html) => html,
        None => return None,
    };
    let body = bytes::Bytes::copy_from_slice(page.as_bytes());
    let mut response_header =
        match pingora::http::ResponseHeader::build(http::StatusCode::NOT_FOUND, None) {
            Ok(h) => h,
            Err(_) => return None,
        };
    if response_header
        .insert_header(http::header::CONTENT_LENGTH, body.len())
        .is_err()
    {
        return None;
    }
    if response_header
        .insert_header(http::header::CONTENT_TYPE, "text/html")
        .is_err()
    {
        return None;
    }
    if session
        .write_response_header(Box::new(response_header))
        .await
        .is_err()
    {
        return None;
    }
    if head_only {
        let _ = session.write_response_body(bytes::Bytes::new(), true).await;
        return None;
    }
    let _ = session.write_response_body(body, true).await;
    None
}
