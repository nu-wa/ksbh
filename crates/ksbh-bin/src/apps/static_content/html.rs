#[derive(askama::Template)]
#[template(path = "pages/error.html")]
pub(super) struct ErrorTemplate<'a> {
    pub inline_css: &'a str,
    pub status_code: &'a str,
    pub title: &'a str,
}

impl<'a> ErrorTemplate<'a> {
    pub(super) fn new(status_code: &'a str, title: &'a str, message: &'a str) -> Self {
        let _ = message;
        Self {
            inline_css: ksbh_ui::SHARED_CSS,
            status_code,
            title,
        }
    }
}
