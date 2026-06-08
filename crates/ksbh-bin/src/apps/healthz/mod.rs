pub struct HealthzApp;

impl HealthzApp {
    pub async fn handle_healthz(
        &self,
        mut session: pingora::protocols::http::ServerSession,
        head_only: bool,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        let body = b"healthzy";

        let mut response_header = pingora::http::ResponseHeader::build(
            pingora::http::StatusCode::OK,
            Some(1),
        )
        .ok()?;

        response_header
            .insert_header(http::header::CONTENT_LENGTH, body.len())
            .ok()?;

        session
            .write_response_header(Box::new(response_header))
            .await
            .ok()?;

        if head_only {
            session
                .write_response_body(bytes::Bytes::new(), true)
                .await
                .ok()?;
        } else {
            session
                .write_response_body(bytes::Bytes::copy_from_slice(body), true)
                .await
                .ok()?;
        }

        None
    }
}
