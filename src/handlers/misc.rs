use crate::AppState;
use crate::auth::AuthUserRedirect;
use askama::Template;
use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::get,
};

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub logged_in: bool,
    pub user_email: String,
}

pub async fn dashboard(AuthUserRedirect(user): AuthUserRedirect) -> impl IntoResponse {
    let template = DashboardTemplate {
        logged_in: true,
        user_email: user.email,
    };
    Html(template.render().unwrap())
}

pub fn router() -> Router<AppState> {
    Router::new().route("/dashboard", get(dashboard))
}
