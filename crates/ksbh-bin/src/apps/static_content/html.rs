#[derive(askama::Template)]
#[template(path = "400.html")]
pub(super) struct Template400;

#[derive(askama::Template)]
#[template(path = "401.html")]
pub(super) struct Template401;

#[derive(askama::Template)]
#[template(path = "403.html")]
pub(super) struct Template403;

#[derive(askama::Template)]
#[template(path = "404.html")]
pub(super) struct Template404;

#[derive(askama::Template)]
#[template(path = "500.html")]
pub(super) struct Template500;

#[derive(askama::Template)]
#[template(path = "502.html")]
pub(super) struct Template502;
