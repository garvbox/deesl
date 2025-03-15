use axum::{extract::State, http::StatusCode, response::IntoResponse};
use deadpool_diesel::postgres::Pool;

pub async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route {}", uri))
}

pub async fn hello_world(State(pool): State<Pool>) -> String {
    let pool_size = pool.status().max_size;
    format!("Hello, World! - Pool size: {pool_size}")
}
