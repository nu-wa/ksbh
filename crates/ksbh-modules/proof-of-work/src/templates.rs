#[derive(askama::Template)]
#[template(path = "challenge.html")]
pub struct ChallengeTemplate {
    pub inline_css: &'static str,
    pub challenge: ::std::string::String,
    pub difficulty: usize,
    pub url: ::std::string::String,
}

pub fn render_challenge(
    challenge: &str,
    difficulty: usize,
    url: &str,
) -> Result<::std::string::String, askama::Error> {
    let template = ChallengeTemplate {
        inline_css: ksbh_ui::SHARED_CSS,
        challenge: challenge.to_string(),
        difficulty,
        url: url.to_string(),
    };
    askama::Template::render(&template)
}
