use crate::AppState;
use askama::Template;
use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::get,
};
use axum_csrf::CsrfToken;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub logged_in: bool,
    pub csrf_token: String,
}

pub async fn login(token: CsrfToken) -> impl IntoResponse {
    let template = LoginTemplate {
        logged_in: false,
        csrf_token: token.authenticity_token().unwrap_or_default(),
    };
    Html(template.render().unwrap())
}

pub fn router() -> Router<AppState> {
    Router::new().route("/login", get(login))
}
