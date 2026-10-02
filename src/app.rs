use std::sync::Arc;

use axum::{Router, routing::get};
use http_security_headers::{
    ContentSecurityPolicy, CrossOriginEmbedderPolicy, CrossOriginOpenerPolicy,
    CrossOriginResourcePolicy, ReferrerPolicy, SecurityHeaders, SecurityHeadersLayer,
};
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::{handlers, oauth_handlers};

async fn serve_version() -> axum::response::Json<serde_json::Value> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    axum::response::Json(serde_json::json!({ "version": VERSION }))
}

async fn health() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({ "status": "ok" }))
}

pub fn build_router(app_state: AppState) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { axum::response::Redirect::to("/dashboard") }),
        )
        .merge(handlers::auth::router())
        .merge(handlers::misc::router())
        .merge(handlers::settings::router())
        .nest("/vehicles", handlers::vehicles::router())
        .nest("/fuel-entries", handlers::fuel_entries::router())
        .nest("/stations", handlers::stations::router())
        .nest("/stats", handlers::stats::router())
        .nest("/import", handlers::import::router())
        .route("/api/version", get(serve_version))
        .route("/health", get(health))
        .merge(oauth_handlers::router())
        .layer(TraceLayer::new_for_http())
        .layer(build_security_headers())
        .with_state(app_state)
}

fn build_security_headers() -> SecurityHeadersLayer {
    let headers = SecurityHeaders::builder()
        .content_security_policy(
            ContentSecurityPolicy::new()
                .default_src(vec!["'self'"])
                .script_src(vec![
                    "'self'",
                    "'unsafe-inline'",
                    "https://unpkg.com/htmx.org@2.0.0/dist/htmx.min.js",
                    "https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js",
                ])
                .style_src(vec!["'self'", "'unsafe-inline'"])
                .img_src(vec!["'self'", "data:", "https://*.googleusercontent.com"])
                .connect_src(vec!["'self'", "https://www.googleapis.com"])
                .font_src(vec!["'self'"])
                .object_src(vec!["'none'"])
                .base_uri(vec!["'self'"])
                .form_action(vec!["'self'", "https://accounts.google.com"])
                .frame_ancestors(vec!["'none'"]),
        )
        .referrer_policy(ReferrerPolicy::StrictOriginWhenCrossOrigin)
        .cross_origin_opener_policy(CrossOriginOpenerPolicy::SameOrigin)
        .cross_origin_embedder_policy(CrossOriginEmbedderPolicy::RequireCorp)
        .cross_origin_resource_policy(CrossOriginResourcePolicy::SameOrigin)
        .build()
        .unwrap();

    SecurityHeadersLayer::new(Arc::new(headers))
}
