pub struct ProfilingHttpApp;

pub fn create_pyroscope_agent(
    config: &ksbh_core::config::Config,
) -> Option<pyroscope::PyroscopeAgent<pyroscope::pyroscope::PyroscopeAgentRunning>> {
    if let Some(url) = &config.pyroscope_url {
        match pyroscope::pyroscope::PyroscopeAgentBuilder::new(url.as_str(), "ksbh")
            .backend(pyroscope_pprofrs::pprof_backend(
                pyroscope_pprofrs::PprofConfig::new().sample_rate(100),
            ))
            .build()
        {
            Ok(built_agent) => match built_agent.start() {
                Ok(running_agent) => {
                    tracing::info!("Started pyroscope agent: {:?}", running_agent);
                    return Some(running_agent);
                }
                Err(e) => {
                    tracing::error!("Failed to start pyroscope agent: {:?}", e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to build pyroscope agent: {:?}", e);
            }
        }
    }

    None
}

fn write_resp(
    body: &[u8],
    status: http::StatusCode,
    content_type: Option<&str>,
    content_encoding: Option<&str>,
) -> http::Response<Vec<u8>> {
    let mut res = http::Response::builder()
        .status(status)
        .header(
            http::header::CONTENT_TYPE,
            content_type.unwrap_or("text/plain"),
        )
        .header(http::header::CONTENT_LENGTH, body.len());

    if let Some(enc) = content_encoding {
        res = res.header(http::header::CONTENT_ENCODING, enc);
    }
    res.body(body.to_vec()).unwrap()
}

#[async_trait::async_trait]
impl pingora::apps::http_app::ServeHttp for ProfilingHttpApp {
    async fn response(
        &self,
        _http_stream: &mut pingora::protocols::http::ServerSession,
    ) -> http::Response<Vec<u8>> {
        let dump = match jemalloc_pprof::PROF_CTL.as_ref() {
            Some(prof_ctl) => match prof_ctl.lock().await.dump_pprof() {
                Err(e) => {
                    tracing::error!("{e}");
                    return write_resp(
                        "Profiling not available".as_bytes(),
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        None,
                        None,
                    );
                }
                Ok(dump) => dump,
            },
            None => {
                return write_resp(
                    "Profiling not available".as_bytes(),
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    None,
                    None,
                );
            }
        };

        write_resp(
            &dump,
            http::StatusCode::OK,
            Some("application/octet-stream"),
            None,
        )
    }
}

pub fn profiling_service()
-> pingora::services::listening::Service<pingora::apps::http_app::HttpServer<ProfilingHttpApp>> {
    let server = pingora::apps::http_app::HttpServer::new_app(ProfilingHttpApp);

    pingora::services::listening::Service::new("profiling_service".to_string(), server)
}
