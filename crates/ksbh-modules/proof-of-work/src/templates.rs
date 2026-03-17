use askama::Template;

#[derive(askama::Template)]
#[template(path = "challenge.html")]
pub struct ChallengeTemplate {
    pub challenge: String,
    pub difficulty: usize,
    pub url: String,
    pub redirect_to: String,
}

pub fn render_challenge(
    challenge: &str,
    difficulty: usize,
    url: &str,
    redirect_to: &str,
) -> Result<String, askama::Error> {
    let template = ChallengeTemplate {
        challenge: challenge.to_string(),
        difficulty,
        url: url.to_string(),
        redirect_to: redirect_to.to_string(),
    };
    template.render()
}
