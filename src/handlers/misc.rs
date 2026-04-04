use crate::AppState;
use crate::auth::AuthUserRedirect;
use askama::Template;
use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::get,
};
use axum_csrf::CsrfToken;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub logged_in: bool,
    pub csrf_token: String,
    pub user_email: String,
}

pub async fn dashboard(
    AuthUserRedirect(user): AuthUserRedirect,
    token: CsrfToken,
) -> impl IntoResponse {
    let template = DashboardTemplate {
        logged_in: true,
        csrf_token: token.authenticity_token().unwrap_or_default(),
        user_email: user.email,
    };
    Html(template.render().unwrap())
}

pub fn router() -> Router<AppState> {
    Router::new().route("/dashboard", get(dashboard))
}
