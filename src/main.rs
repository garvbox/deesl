use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use deadpool_diesel::postgres::{Manager, Pool};
use std::env;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing::info;

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
        .route("/", get(hello_world))
        .fallback(not_found)
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

async fn hello_world(State(pool): State<deadpool_diesel::postgres::Pool>) -> String {
    let pool_size = pool.status().max_size;
    format!("Hello, World! - Pool size: {pool_size}")
}

async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route {}", uri))
}
