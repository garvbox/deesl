use axum::{
    Router, http::Method, http::Request, http::header, middleware, response::IntoResponse,
    routing::get,
};
use deadpool_diesel::postgres::{Manager, Pool};
use http_security_headers::{
    ContentSecurityPolicy, CrossOriginEmbedderPolicy, CrossOriginOpenerPolicy,
    CrossOriginResourcePolicy, ReferrerPolicy, SecurityHeaders, SecurityHeadersLayer,
};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing::info;
use utoipa::OpenApi;

use deesl::{
    AppState, api_doc, oauth_handlers, user_handlers, vehicle_fuel_handlers, vehicle_share_handlers,
};

async fn serve_openapi() -> axum::response::Json<String> {
    axum::response::Json(api_doc::ApiDoc::openapi().to_json().unwrap())
}

async fn serve_version() -> axum::response::Json<serde_json::Value> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    axum::response::Json(serde_json::json!({ "version": VERSION }))
}

async fn serve_index() -> impl IntoResponse {
    match tokio::fs::read_to_string("src/pkg/index.html").await {
        Ok(content) => axum::response::Html(content),
        Err(err) => {
            tracing::error!("Failed to read index.html: {}", err);
            axum::response::Html(
                "<h1>Server Error</h1><p>Failed to load application. Please try again later.</p>"
                    .to_string(),
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    port: usize,
    host: String,
    database_url: String,
    environment: String,
    cors_origins: Vec<String>,
    base_url: String,
}

impl Config {
    fn new() -> Self {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .expect("PORT must be a number"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            database_url,
            environment,
            cors_origins,
            base_url,
        }
    }

    fn is_development(&self) -> bool {
        self.environment == "development"
    }

    #[cfg(test)]
    fn with_environment(environment: &str, cors_origins: Vec<String>) -> Self {
        Self {
            port: 8000,
            host: "localhost".to_string(),
            database_url: String::new(),
            environment: environment.to_string(),
            cors_origins,
            base_url: "http://localhost:8000".to_string(),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let _ = dotenvy::dotenv();
    let config = Config::new();

    let manager = Manager::new(&config.database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();
    let app_state = AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::new(&config.base_url),
    };

    let mut app = Router::new()
        .route("/api/version", get(serve_version))
        .route("/api/openapi.json", get(serve_openapi))
        .merge(oauth_handlers::router())
        .merge(user_handlers::router())
        .merge(vehicle_fuel_handlers::router())
        .merge(vehicle_share_handlers::router())
        .nest_service("/assets", ServeDir::new("src/pkg/assets"))
        .fallback(serve_index)
        .layer(middleware::from_fn(add_cache_control_headers))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    if config.is_development() {
        let cors = CorsLayer::new()
            .allow_origin(AllowOrigin::mirror_request())
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE, header::ACCEPT, header::ORIGIN])
            .allow_credentials(true);
        app = app.layer(cors).layer(LiveReloadLayer::new());
    } else {
        let cors = build_production_cors(&config.cors_origins);
        let security_headers = build_security_headers();
        app = app.layer(cors).layer(security_headers);
    }

    let bind_address = format!("{}:{}", config.host, config.port);
    info!("Starting server on http://{bind_address}");
    let listener = TcpListener::bind(bind_address)
        .await
        .expect("Failed to bind listener");
    axum::serve(listener, app)
        .await
        .expect("Failed to start axum server");
}

fn build_production_cors(origins: &[String]) -> CorsLayer {
    let allowed_headers = [header::CONTENT_TYPE, header::ACCEPT, header::ORIGIN];
    let allowed_methods = vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ];

    if origins.is_empty() {
        CorsLayer::new()
            .allow_origin(AllowOrigin::mirror_request())
            .allow_methods(allowed_methods)
            .allow_headers(allowed_headers)
            .allow_credentials(true)
    } else {
        let origins: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(allowed_methods)
            .allow_headers(allowed_headers)
            .allow_credentials(true)
    }
}

fn build_security_headers() -> SecurityHeadersLayer {
    let csp = ContentSecurityPolicy::new()
        .default_src(vec!["'self'"])
        .script_src(vec!["'self'"])
        .style_src(vec!["'self'", "'unsafe-inline'"])
        .img_src(vec!["'self'", "data:"])
        .font_src(vec!["'self'"])
        .connect_src(vec!["'self'"])
        .frame_ancestors(vec!["'none'"])
        .base_uri(vec!["'self'"])
        .form_action(vec!["'self'"]);

    let headers = SecurityHeaders::builder()
        .strict_transport_security(Duration::from_secs(31536000), true, true)
        .content_security_policy(csp)
        .referrer_policy(ReferrerPolicy::StrictOriginWhenCrossOrigin)
        .cross_origin_resource_policy(CrossOriginResourcePolicy::SameOrigin)
        .cross_origin_opener_policy(CrossOriginOpenerPolicy::SameOrigin)
        .cross_origin_embedder_policy(CrossOriginEmbedderPolicy::RequireCorp)
        .x_content_type_options_nosniff()
        .x_frame_options_deny()
        .build()
        .unwrap();

    SecurityHeadersLayer::new(Arc::new(headers))
}

async fn add_cache_control_headers(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let path = request.uri().path().to_owned();
    let mut response = next.run(request).await;

    let cache_header = if path == "/assets/manifest.json" {
        // Manifest changes every build - never cache
        "no-cache, no-store, must-revalidate"
    } else if path.starts_with("/assets/") && path.contains('-') {
        // Hashed assets (e.g., index-BN4_6Tgn.js) - cache forever
        "public, max-age=31536000, immutable"
    } else {
        // Everything else - use browser defaults
        return response;
    };

    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, cache_header.parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_is_development_when_environment_is_development() {
        let config = Config::with_environment("development", vec![]);
        assert!(config.is_development());
    }

    #[test]
    fn test_config_is_not_development_when_environment_is_production() {
        let config = Config::with_environment("production", vec![]);
        assert!(!config.is_development());
    }

    #[test]
    fn test_config_is_not_development_when_environment_is_staging() {
        let config = Config::with_environment("staging", vec![]);
        assert!(!config.is_development());
    }
}
