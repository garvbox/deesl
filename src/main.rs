use axum::{
    Router, http::Method, http::Request, http::header, middleware, response::IntoResponse,
    routing::{delete, get, post},
};
use deadpool_diesel::postgres::{Manager, Pool};
use diesel::prelude::*;
use http_security_headers::{
    ContentSecurityPolicy, CrossOriginEmbedderPolicy, CrossOriginOpenerPolicy,
    CrossOriginResourcePolicy, ReferrerPolicy, SecurityHeaders, SecurityHeadersLayer,
};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing::info;
use utoipa::OpenApi;

use deesl::{
    AppState, api_doc, auth::DEV_AUTH_EMAIL_KEY, handlers, import_handlers, models::NewUser,
    oauth_handlers, schema::users, user_handlers, vehicle_fuel_handlers, vehicle_share_handlers,
};

async fn serve_openapi() -> axum::response::Json<String> {
    axum::response::Json(api_doc::ApiDoc::openapi().to_json().unwrap())
}

async fn serve_version() -> axum::response::Json<serde_json::Value> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    axum::response::Json(serde_json::json!({ "version": VERSION }))
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

async fn ensure_dev_user_exists(pool: &Pool) {
    // Only create dev user in debug builds
    if cfg!(not(debug_assertions)) {
        return;
    }

    let Ok(email) = env::var(DEV_AUTH_EMAIL_KEY) else {
        return;
    };

    let conn = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to get DB connection for dev user creation: {}", e);
            return;
        }
    };

    let email_clone = email.clone();
    let result = conn.interact(move |conn| {
        use diesel::dsl::exists;

        // Check if user already exists
        let user_exists: bool = diesel::select(exists(
            users::table.filter(users::email.eq(&email_clone))
        )).get_result(conn)?;

        if user_exists {
            tracing::info!("Dev user {} already exists", email_clone);
            return Ok::<(), diesel::result::Error>(());
        }

        // Try to insert with ID 1 first (for auth bypass compatibility)
        let insert_with_id_result = diesel::sql_query(
            "INSERT INTO users (id, email, password_hash, currency, google_id) \
             VALUES (1, $1, NULL, 'EUR', NULL) \
             ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email"
        )
        .bind::<diesel::sql_types::Text, _>(&email_clone)
        .execute(conn);

        match insert_with_id_result {
            Ok(_) => {
                tracing::info!("Created dev user with ID 1: {}", email_clone);
            }
            Err(_) => {
                // ID 1 is taken, insert normally
                let new_user = NewUser {
                    email: email_clone.clone(),
                    password_hash: None,
                    currency: "EUR".to_string(),
                    google_id: None,
                };

                diesel::insert_into(users::table)
                    .values(&new_user)
                    .execute(conn)?;

                tracing::warn!("Created dev user {} with auto-generated ID (expected ID 1 for auth bypass)", email_clone);
            }
        }

        Ok(())
    }).await;

    if let Err(e) = result {
        tracing::warn!("Failed to create dev user: {}", e);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let _ = dotenvy::dotenv();
    let config = Config::new();

    let manager = Manager::new(&config.database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();

    // Ensure dev user exists if DEV_AUTH_EMAIL is set (debug builds only)
    ensure_dev_user_exists(&pool).await;

    let app_state = AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::new(&config.base_url),
    };

    let mut app = Router::new()
        .route("/", get(|| async { axum::response::Redirect::to("/dashboard") }))
        .route("/login", get(handlers::login))
        .route("/dashboard", get(handlers::dashboard))
        .route("/vehicles", get(handlers::vehicles_page).post(handlers::create_vehicle))
        .route("/vehicles/new", get(handlers::new_vehicle))
        .route("/fuel-entries/new", get(handlers::new_fuel_entry))
        .route("/fuel-entries", post(handlers::create_fuel_entry))
        .route("/htmx/vehicles", get(handlers::htmx_vehicles))
        .route("/htmx/vehicles/{id}", delete(handlers::htmx_delete_vehicle))
        .route("/htmx/entries/recent", get(handlers::htmx_recent_entries))
        .route("/api/version", get(serve_version))
        .route("/api/openapi.json", get(serve_openapi))
        .merge(oauth_handlers::router())
        .merge(user_handlers::router())
        .merge(vehicle_fuel_handlers::router())
        .merge(vehicle_share_handlers::router())
        .merge(import_handlers::router())
        .fallback_service(ServeDir::new("src/pkg").fallback(ServeFile::new("src/pkg/index.html")))
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
