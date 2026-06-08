const ERROR_PAGES: &[(u16, &str)] = &[
    (400, "400"),
    (401, "401"),
    (403, "403"),
    (404, "404"),
    (500, "500"),
    (502, "502"),
];

#[derive(Debug)]
pub enum ErrorPagesAppError {
    Internal(ksbh_types::KsbhStr),
}

impl ::std::error::Error for ErrorPagesAppError {}

impl ::std::fmt::Display for ErrorPagesAppError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "ErrorPagesAppError {}",
            match self {
                Self::Internal(m) => m,
            }
        )
    }
}

pub struct ErrorPagesApp {
    templates: scc::HashMap<&'static str, String>,
}

impl ErrorPagesApp {
    pub fn new() -> Result<Self, ErrorPagesAppError> {
        let templates = scc::HashMap::with_capacity(7);

        for page in ["400", "401", "403", "404", "405", "500", "502"] {
            let rendered = ksbh_ui::error_pages::render_error_page_html(page).ok_or_else(|| {
                ErrorPagesAppError::Internal(ksbh_types::KsbhStr::new(format!(
                    "missing or failed to render static error template for code {page}"
                )))
            })?;
            templates.upsert_sync(page, rendered);
        }

        Ok(Self { templates })
    }

    pub async fn send(
        &self,
        mut session: pingora::protocols::http::ServerSession,
        code: u16,
        head_only: bool,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        let page = ERROR_PAGES.iter().find(|(c, _)| *c == code)?.1;
        let body = bytes::Bytes::copy_from_slice(self.templates.get_sync(page)?.as_bytes());
        let status = http::StatusCode::from_u16(code).ok()?;
        let mut response_header = pingora::http::ResponseHeader::build(status, None).ok()?;

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

        if head_only {
            session
                .write_response_body(bytes::Bytes::new(), true)
                .await
                .ok()?;
            return None;
        }

        session.write_response_body(body, true).await.ok()?;

        None
    }

    pub async fn send_405(
        &self,
        mut session: pingora::protocols::http::ServerSession,
        head_only: bool,
    ) -> Option<pingora::apps::ReusedHttpStream> {
        let body = bytes::Bytes::copy_from_slice(self.templates.get_sync("405")?.as_bytes());
        let mut response_header =
            pingora::http::ResponseHeader::build(http::StatusCode::METHOD_NOT_ALLOWED, None)
                .ok()?;

        response_header
            .insert_header(http::header::CONTENT_LENGTH, body.len())
            .ok()?;
        response_header
            .insert_header(http::header::CONTENT_TYPE, "text/html")
            .ok()?;
        response_header
            .insert_header(http::header::ALLOW, "GET, HEAD")
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
            return None;
        }

        session.write_response_body(body, true).await.ok()?;

        None
    }
}
