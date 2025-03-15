use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;

use crate::{models, schema};

pub async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("No route {}", uri))
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    vehicles: &'a Vec<models::Vehicle>,
}

pub async fn list_vehicles(State(pool): State<Pool>) -> Result<Html<String>, (StatusCode, String)> {
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

    let template = IndexTemplate { vehicles: &res };
    Ok(Html(template.render().unwrap()))
}

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
