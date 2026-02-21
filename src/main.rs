use axum::{
    routing::{get, post},
    Router,
};
use deadpool_diesel::postgres::{Manager, Pool};
use std::env;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use tower_livereload::LiveReloadLayer;
use tracing::info;
use utoipa::OpenApi;

mod api_doc;
mod auth;
mod auth_handlers;
mod handlers;
mod models;
mod schema;
mod vehicle_fuel_handlers;

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
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let _ = dotenvy::dotenv();
    let config = Config::new();

    // set up connection pool
    let manager = Manager::new(&config.database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();

    let app = Router::new()
        .nest_service("/static", ServeDir::new("src/pkg"))
        .route("/api/openapi.json", get(serve_openapi))
        .merge(auth_handlers::router())
        .merge(vehicle_fuel_handlers::router())
        .route(
            "/vehicles",
            get(handlers::list_vehicles).post(handlers::add_new_vehicle),
        )
        .route("/vehicles/{vehicle_id}", post(handlers::update_vehicle))
        .fallback(serve_index)
        .layer(TraceLayer::new_for_http())
        .layer(LiveReloadLayer::new())
        .with_state(pool);

    let bind_address = format!("{}:{}", config.host, config.port);
    info!("Starting server on http://{bind_address}");
    let listener = TcpListener::bind(bind_address)
        .await
        .expect("Failed to bind listener");
    axum::serve(listener, app)
        .await
        .expect("Failed to start axum server");
}
