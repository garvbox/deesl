use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use tokio::net::TcpListener;
use tower_livereload::LiveReloadLayer;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(hello_world))
        .fallback(not_found)
        .layer(LiveReloadLayer::new());

    info!("Starting server on http://localhost:8000");
    let listener = TcpListener::bind("0.0.0.0:8000")
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
