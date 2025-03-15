use axum::{extract::State, http::StatusCode, response::IntoResponse, response::Json};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;

use crate::{models, schema};

pub async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route {}", uri))
}

pub async fn list_vehicles(
    State(pool): State<Pool>,
) -> Result<Json<Vec<models::Vehicle>>, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let res = conn
        .interact(|conn| {
            schema::vehicles::table
                .select(models::Vehicle::as_select())
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    Ok(Json(res))
}

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
