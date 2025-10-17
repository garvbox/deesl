use askama::Template;
use axum::{
    Form,
    extract::{self, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::Deserialize;

use crate::{
    models::{self, NewVehicle},
    schema::{self, vehicles},
};

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

pub async fn add_new_vehicle(
    State(pool): State<Pool>,
    Form(payload): Form<NewVehicle>,
) -> Result<Redirect, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    conn.interact(|conn| {
        payload
            .insert_into(vehicles::table)
            .returning(models::Vehicle::as_returning())
            .get_result(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;

    Ok(Redirect::to("/vehicles"))
}

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
