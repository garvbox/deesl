use crate::AppState;
use askama::Template;
use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::get,
};

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub logged_in: bool,
}

pub async fn login() -> impl IntoResponse {
    let template = LoginTemplate { logged_in: false };
    Html(template.render().unwrap())
}

pub fn router() -> Router<AppState> {
    Router::new().route("/login", get(login))
}
