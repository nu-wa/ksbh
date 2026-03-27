#[derive(askama::Template)]
#[template(path = "pages/error.html")]
struct ErrorTemplate<'a> {
    inline_css: &'a str,
    status_code: &'a str,
    title: &'a str,
}

fn error_template_metadata(page: &str) -> Option<(&'static str, &'static str)> {
    match page {
        "400" => Some((
            "Bad Request",
            "The request could not be parsed or accepted by the static content app.",
        )),
        "401" => Some((
            "Unauthorized",
            "Authentication is required before this resource can be returned.",
        )),
        "403" => Some((
            "Forbidden",
            "The request was understood, but this resource is not available to the current client.",
        )),
        "404" => Some((
            "Not Found",
            "No static asset or matching page was found for this request path.",
        )),
        "405" => Some((
            "Method Not Allowed",
            "This static endpoint only accepts GET and HEAD requests.",
        )),
        "500" => Some((
            "Internal Server Error",
            "The static content app failed while preparing a response.",
        )),
        "502" => Some((
            "Bad Gateway",
            "The proxy could not get a valid upstream response for this request.",
        )),
        _ => {
            let status = page.parse::<u16>().ok()?;
            if (400..=599).contains(&status) {
                Some((
                    "Upstream Error",
                    "The upstream returned an error response without a response body.",
                ))
            } else {
                None
            }
        }
    }
}

pub fn render_error_page_html(page: &str) -> Option<String> {
    let (title, message) = error_template_metadata(page)?;
    let _ = message;
    askama::Template::render(&ErrorTemplate {
        inline_css: crate::SHARED_CSS,
        status_code: page,
        title,
    })
    .ok()
}
