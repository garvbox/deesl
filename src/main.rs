use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use std::env;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing::info;

#[derive(Debug)]
pub struct Config {
    port: usize,
    host: String,
}

impl Config {
    fn new() -> Self {
        Self {
            port: env::var("PORT")
                .unwrap_or("8000".to_string())
                .parse()
                .unwrap(),
            host: env::var("HOST").unwrap_or("localhost".to_string()),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(hello_world))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(LiveReloadLayer::new());

    let config = Config::new();

    let bind_address = format!("{}:{}", config.host, config.port);
    info!("Starting server on http://{bind_address}");
    let listener = TcpListener::bind(bind_address)
        .await
        .expect("Failed to bind listener");
    axum::serve(listener, app)
        .await
        .expect("Failed to start axum server");
}

pub async fn hello_world() -> &'static str {
    "Hello, World!"
}

async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route {}", uri))
}
