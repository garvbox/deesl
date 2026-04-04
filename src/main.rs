use axum::{Router, routing::get};
use deadpool_diesel::postgres::{Manager, Pool};
use http_security_headers::{
    ContentSecurityPolicy, CrossOriginEmbedderPolicy, CrossOriginOpenerPolicy,
    CrossOriginResourcePolicy, ReferrerPolicy, SecurityHeaders, SecurityHeadersLayer,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

#[cfg(feature = "dev")]
use tower_livereload::LiveReloadLayer;

use deesl::{AppConfig, AppError, AppState, handlers, oauth_handlers};

async fn serve_version() -> axum::response::Json<serde_json::Value> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    axum::response::Json(serde_json::json!({ "version": VERSION }))
}

async fn health() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({ "status": "ok" }))
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env().expect("Failed to load configuration");

    let manager = Manager::new(&config.database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager)
        .max_size(10)
        .build()
        .expect("Failed to create pool");

    if let Err(err) = run_migrations(&pool).await {
        tracing::error!("Failed to run migrations: {:?}", err);
        return;
    }

    #[cfg(feature = "dev")]
    if let Err(err) = setup_dev_auth_user(&pool, config.dev_auth_email).await {
        tracing::error!("Failed to set up dev user: {:?}", err);
        return;
    }

    let app_state = AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::new(
            &config.google_client_id,
            &config.google_client_secret,
            &config.base_url,
        ),
        auth: deesl::auth::AuthConfig::new(&config.jwt_secret, config.jwt_expiration_hours),
    };

    let app = Router::new()
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
        .with_state(app_state);

    #[cfg(feature = "dev")]
    let app = app.layer(LiveReloadLayer::new());

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

async fn run_migrations(pool: &Pool) -> Result<(), AppError> {
    let conn = pool.get().await?;

    conn.interact(|conn| {
        use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
        const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
        conn.run_pending_migrations(MIGRATIONS)
            .expect("Failed to run migrations");
    })
    .await
    .expect("Failed to interact with database");

    Ok(())
}

#[cfg(feature = "dev")]
async fn setup_dev_auth_user(pool: &Pool, dev_auth_email: Option<String>) -> Result<(), AppError> {
    tracing::trace!(
        "checking dev auth user for bypass exists: {:?}",
        dev_auth_email
    );

    if let Some(dev_auth_email) = dev_auth_email {
        deesl::user::create_user_if_not_exists(
            pool,
            deesl::models::NewUser {
                email: dev_auth_email,
                password_hash: None,
                google_id: None,
                currency: "EUR".to_string(),
                distance_unit: "km".to_string(),
                volume_unit: "L".to_string(),
            },
        )
        .await?;
    };

    Ok(())
}
