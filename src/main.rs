use axum::{
    Router,
    routing::{get, post},
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
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing::info;
use utoipa::OpenApi;

mod api_doc;
mod auth;
mod handlers;
mod models;
mod oauth_handlers;
mod schema;
mod state;
mod user_handlers;
mod vehicle_fuel_handlers;

pub use state::AppState;

async fn serve_openapi() -> axum::response::Json<String> {
    axum::response::Json(api_doc::ApiDoc::openapi().to_json().unwrap())
}

async fn serve_index() -> impl axum::response::IntoResponse {
    axum::response::Html(include_str!("pkg/index.html"))
}

#[derive(Debug)]
pub struct Config {
    port: usize,
    host: String,
    database_url: String,
    environment: String,
    cors_origins: Vec<String>,
}

impl Config {
    fn new() -> Self {
        Self {
            port: env::var("PORT")
                .unwrap_or("8000".to_string())
                .parse()
                .unwrap(),
            host: env::var("HOST").unwrap_or("localhost".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or("postgres://postgres:postgres@localhost/deesl".to_string()),
            environment: env::var("ENVIRONMENT").unwrap_or("development".to_string()),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
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
        oauth: oauth_handlers::OAuthConfig::new(),
    };

    let mut app = Router::new()
        .route("/api/openapi.json", get(serve_openapi))
        .merge(oauth_handlers::router())
        .merge(user_handlers::router())
        .merge(vehicle_fuel_handlers::router())
        .route(
            "/vehicles",
            get(handlers::list_vehicles).post(handlers::add_new_vehicle),
        )
        .route("/vehicles/{vehicle_id}", post(handlers::update_vehicle))
        .nest_service("/assets", ServeDir::new("src/pkg/assets"))
        .fallback(serve_index)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    if config.is_development() {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
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
    if origins.is_empty() {
        CorsLayer::new()
    } else {
        let origins: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(Any)
            .allow_headers(Any)
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- Config::is_development ---

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

    #[test]
    fn test_config_is_not_development_when_environment_is_empty() {
        let config = Config::with_environment("", vec![]);
        assert!(!config.is_development());
    }

    // --- build_production_cors ---

    #[test]
    fn test_build_production_cors_with_empty_origins_returns_layer() {
        // Should not panic; returns a restrictive (no-allow) layer
        let _layer = build_production_cors(&[]);
    }

    #[test]
    fn test_build_production_cors_with_valid_origins_returns_layer() {
        let origins = vec![
            "https://example.com".to_string(),
            "https://app.example.com".to_string(),
        ];
        let _layer = build_production_cors(&origins);
    }

    #[test]
    fn test_build_production_cors_ignores_invalid_origin_strings() {
        // Invalid origins are filtered out by `filter_map(|o| o.parse().ok())`
        let origins = vec!["not a valid origin".to_string()];
        let _layer = build_production_cors(&origins);
    }
}
