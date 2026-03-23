mod errors;
mod file_cache;
mod html;

#[derive(PartialEq, Eq)]
enum Compression {
    Gzip,
    Zlib,
    Deflate,
    Brotli,
    None,
}

pub struct StaticHttpApp {
    config: ::std::sync::Arc<ksbh_core::Config>,
    templates: scc::HashMap<&'static str, String>,
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

impl StaticHttpApp {
    pub fn new(
        config: ::std::sync::Arc<ksbh_core::Config>,
    ) -> Result<Self, errors::StaticHttpAppError> {
        use askama::Template;
        let templates = scc::HashMap::with_capacity(6);

        templates.upsert_sync(
            "400",
            html::ErrorTemplate::new(
                "400",
                "Bad Request",
                "The request could not be parsed or accepted by the static content app.",
            )
            .render()?,
        );
        templates.upsert_sync(
            "401",
            html::ErrorTemplate::new(
                "401",
                "Unauthorized",
                "Authentication is required before this resource can be returned.",
            )
            .render()?,
        );
        templates.upsert_sync(
            "403",
            html::ErrorTemplate::new("403", "Forbidden", "The request was understood, but this resource is not available to the current client.").render()?,
        );
        templates.upsert_sync(
            "404",
            html::ErrorTemplate::new(
                "404",
                "Not Found",
                "No static asset or matching page was found for this request path.",
            )
            .render()?,
        );
        templates.upsert_sync(
            "500",
            html::ErrorTemplate::new(
                "500",
                "Internal Server Error",
                "The static content app failed while preparing a response.",
            )
            .render()?,
        );
        templates.upsert_sync(
            "502",
            html::ErrorTemplate::new(
                "502",
                "Bad Gateway",
                "The proxy could not get a valid upstream response for this request.",
            )
            .render()?,
        );

        Ok(Self {
            config: config.clone(),
            templates,
            file_cache: file_cache::FileCache::new(),
        })
    }

    async fn send_400(
        &self,
        session: pingora::protocols::http::ServerSession,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        self.send_error_page(session, "400", http::StatusCode::BAD_REQUEST)
            .await
    }

    async fn send_401(
        &self,
        session: pingora::protocols::http::ServerSession,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        self.send_error_page(session, "401", http::StatusCode::UNAUTHORIZED)
            .await
    }

    async fn send_403(
        &self,
        session: pingora::protocols::http::ServerSession,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        self.send_error_page(session, "403", http::StatusCode::FORBIDDEN)
            .await
    }

    async fn send_404(
        &self,
        session: pingora::protocols::http::ServerSession,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        self.send_error_page(session, "404", http::StatusCode::NOT_FOUND)
            .await
    }

    async fn send_500(
        &self,
        session: pingora::protocols::http::ServerSession,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        self.send_error_page(session, "500", http::StatusCode::INTERNAL_SERVER_ERROR)
            .await
    }

    async fn send_502(
        &self,
        session: pingora::protocols::http::ServerSession,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        self.send_error_page(session, "502", http::StatusCode::BAD_GATEWAY)
            .await
    }

    async fn send_error_page(
        &self,
        mut session: pingora::protocols::http::ServerSession,
        page: &str,
        code: http::StatusCode,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        let body = bytes::Bytes::copy_from_slice(self.templates.get_sync(page)?.as_bytes());
        let mut response_header = pingora::http::ResponseHeader::build(code, None).ok()?;

        response_header
            .insert_header(http::header::CONTENT_LENGTH, body.len())
            .ok()?;
        response_header
            .insert_header(http::header::CONTENT_TYPE, "text/html")
            .ok()?;

        session
            .write_response_header(Box::new(response_header))
            .await
            .ok()?;

        session.write_response_body(body, true).await.ok()?;

        None
    }

    async fn render_static_file(
        &self,
        mut session: pingora::protocols::http::ServerSession,
        _shutdown: &pingora::server::ShutdownWatch,
        file_param: Option<&str>,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        let file_param = file_param?;
        let decoded = urlencoding::decode(file_param).ok()?;
        let file_path = get_clean_file_path(&self.config.config_paths.static_content, &decoded)?;

        let file_meta = self.file_cache.get(&file_path).await?;

        if let Some(if_none) = session.get_header("if-none-match")
            && if_none.as_bytes() == file_meta.etag.as_bytes()
        {
            let mut response_header = pingora::http::ResponseHeader::build(
                pingora::http::StatusCode::UNAUTHORIZED,
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

#[async_trait::async_trait]
impl pingora::apps::HttpServerApp for StaticHttpApp {
    async fn process_new_http(
        self: &::std::sync::Arc<Self>,
        mut session: pingora::protocols::http::ServerSession,
        shutdown: &pingora::server::ShutdownWatch,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        tracing::span!(tracing::Level::DEBUG, "StaticHttpApp_process_new_http");
        match session.read_request().await {
            Ok(success) => {
                tracing::debug!("StaticHttpApp: read_request: {success}");
            }
            Err(e) => {
                tracing::error!("{:?}", e);

                return None;
            }
        };

        let req_id = uuid::Uuid::new_v4();
        let req_headers = session.req_header();

        let http_request_info = match ksbh_types::requests::http_request::HttpRequestView::new(
            req_headers,
            req_id,
            &self.config.ports.external,
        ) {
            Ok(info) => info,
            Err(e) => {
                tracing::error!("{:?}", e);

                return None;
            }
        };

        // Extract all data we need from HttpRequestView into owned types
        let method_str = http_request_info.method.0;
        let path = http_request_info.query.path.to_string();
        let file_param = http_request_info
            .query
            .get_param("file")
            .map(|s| s.to_string());

        tracing::debug!("http_request_info: method={:?} path={}", method_str, path);

        if method_str == "GET" {
            match path.as_str() {
                "/healthz" => {
                    let res = b"healthzy";

                    let mut response_header = pingora::http::ResponseHeader::build(
                        pingora::http::StatusCode::OK,
                        Some(1),
                    )
                    .ok()?;

                    response_header
                        .insert_header(http::header::CONTENT_LENGTH, res.len())
                        .ok()?;

                    session
                        .write_response_header(Box::new(response_header))
                        .await
                        .ok()?;

                    session
                        .write_response_body(bytes::Bytes::copy_from_slice(res), true)
                        .await
                        .ok()?;

                    return None;
                }
                "/static" => {
                    return self
                        .render_static_file(session, shutdown, file_param.as_deref())
                        .await;
                }
                "/400" => {
                    return self.send_400(session).await;
                }
                "/401" => {
                    return self.send_401(session).await;
                }
                "/403" => {
                    return self.send_403(session).await;
                }
                "/502" => {
                    return self.send_502(session).await;
                }
                "/500" => {
                    return self.send_500(session).await;
                }
                _ => {
                    return self.send_404(session).await;
                }
            };
        }

        self.send_404(session).await
    }
}

pub fn static_http_service(
    config: ::std::sync::Arc<ksbh_core::Config>,
) -> pingora::services::listening::Service<StaticHttpApp> {
    pingora::services::listening::Service::new(
        "static_service".to_string(),
        StaticHttpApp::new(config).expect("Could not create StaticHttpApp"),
    )
}
