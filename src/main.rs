use axum::{
    Router,
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
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing::info;

use deesl::{
    AppState, auth::DEV_AUTH_EMAIL_KEY, handlers, models::NewUser, oauth_handlers, schema::users,
};

async fn serve_version() -> axum::response::Json<serde_json::Value> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    axum::response::Json(serde_json::json!({ "version": VERSION }))
}

#[derive(Debug, Clone)]
pub struct Config {
    port: usize,
    host: String,
    database_url: String,
    base_url: String,
    environment: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .expect("PORT must be a number"),
            host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            base_url: env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string()),
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        }
    }

    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();

    let manager = Manager::new(&config.database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager)
        .max_size(10)
        .build()
        .expect("Failed to create pool");

    // Run migrations
    let conn = pool
        .get()
        .await
        .expect("Failed to get connection from pool");
    conn.interact(|conn| {
        use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
        const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
        conn.run_pending_migrations(MIGRATIONS)
            .expect("Failed to run migrations");
    })
    .await
    .expect("Failed to interact with database");

    // Ensure dev user exists if dev auth is enabled
    if config.is_development() && env::var(DEV_AUTH_EMAIL_KEY).is_ok() {
        let dev_email = env::var(DEV_AUTH_EMAIL_KEY).unwrap();
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            if let Ok(conn) = pool_clone.get().await {
                let _ = conn
                    .interact(move |conn| {
                        let exists = users::table
                            .filter(users::email.eq(&dev_email))
                            .first::<deesl::models::User>(conn)
                            .is_ok();

                        if !exists {
                            let _ = diesel::insert_into(users::table)
                                .values(NewUser {
                                    email: dev_email,
                                    password_hash: None,
                                    google_id: None,
                                    currency: "EUR".to_string(),
                                })
                                .execute(conn);
                        }
                    })
                    .await;
            }
        });
    }

    let app_state = AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::new(&config.base_url),
    };

    let mut app = Router::new()
        .route(
            "/",
            get(|| async { axum::response::Redirect::to("/dashboard") }),
        )
        .route("/login", get(handlers::login))
        .route("/logout", get(oauth_handlers::logout))
        .route("/dashboard", get(handlers::dashboard))
        .route(
            "/settings",
            get(handlers::settings_page).patch(handlers::update_settings),
        )
        .route(
            "/vehicles",
            get(handlers::vehicles_page).post(handlers::create_vehicle),
        )
        .route("/vehicles/new", get(handlers::new_vehicle))
        .route("/fuel-entries/new", get(handlers::new_fuel_entry))
        .route("/fuel-entries", post(handlers::create_fuel_entry))
        .route("/fuel-entries/{id}/edit", get(handlers::edit_fuel_entry))
        .route("/fuel-entries/{id}", post(handlers::update_fuel_entry))
        .route(
            "/stations",
            get(handlers::stations_page).post(handlers::create_station),
        )
        .route("/stations/{id}", post(handlers::update_station))
        .route("/stations/{id}", delete(handlers::delete_station))
        .route("/import", get(handlers::import_page))
        .route("/htmx/import/preview", post(handlers::htmx_import_preview))
        .route("/htmx/import/execute", post(handlers::htmx_import_execute))
        .route("/htmx/vehicles", get(handlers::htmx_vehicles))
        .route("/htmx/vehicles/{id}", delete(handlers::htmx_delete_vehicle))
        .route("/htmx/entries/recent", get(handlers::htmx_recent_entries))
        .route("/htmx/stations/search", get(handlers::htmx_station_search))
        .route("/api/version", get(serve_version))
        .merge(oauth_handlers::router())
        .layer(TraceLayer::new_for_http())
        .layer(build_security_headers())
        .with_state(app_state);

    if config.is_development() {
        app = app.layer(LiveReloadLayer::new());
    }

    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await.unwrap();
    info!("listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
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

    impl Config {
        #[cfg(test)]
        pub fn with_environment(env: &str, _cors: Vec<String>) -> Self {
            Self {
                port: 8000,
                host: "127.0.0.1".to_string(),
                database_url: "postgres://localhost/deesl".to_string(),
                base_url: "http://localhost:8000".to_string(),
                environment: env.to_string(),
            }
        }
    }
}
