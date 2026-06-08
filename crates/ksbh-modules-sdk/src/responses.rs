pub fn text_response(status: http::StatusCode, body: impl Into<String>) -> crate::RequestResult {
    Ok(crate::ModuleResult::Stop(Some(
        http::Response::builder()
            .status(status)
            .body(bytes::Bytes::from(body.into()))?,
    )))
}

pub fn plain_text_response(
    status: http::StatusCode,
    body: impl Into<String>,
) -> crate::RequestResult {
    Ok(crate::ModuleResult::Stop(Some(
        http::Response::builder()
            .status(status)
            .header(http::header::CONTENT_TYPE, "text/plain")
            .body(bytes::Bytes::from(body.into()))?,
    )))
}

pub fn redirect_response(location: &str) -> crate::RequestResult {
    Ok(crate::ModuleResult::Stop(Some(
        http::Response::builder()
            .status(http::StatusCode::FOUND)
            .header(http::header::LOCATION, location)
            .header(
                http::header::CACHE_CONTROL,
                "no-store, no-cache, must-revalidate, max-age=0",
            )
            .body(bytes::Bytes::new())?,
    )))
}

pub fn empty_response(status: http::StatusCode) -> crate::RequestResult {
    Ok(crate::ModuleResult::Stop(Some(
        http::Response::builder()
            .status(status)
            .body(bytes::Bytes::new())?,
    )))
}
