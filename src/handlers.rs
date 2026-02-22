use askama::Template;
use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;

use crate::{
    models::{self, NewVehicle},
    schema,
};

#[allow(dead_code)]
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
            .insert_into(schema::vehicles::table)
            .returning(models::Vehicle::as_returning())
            .get_result(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;

    Ok(Redirect::to("/vehicles"))
}

#[derive(serde::Deserialize)]
pub struct UpdateVehicle {
    pub action: String,
}

pub async fn update_vehicle(
    State(pool): State<Pool>,
    Path(vehicle_id): Path<i32>,
    Form(payload): Form<UpdateVehicle>,
) -> Result<Redirect, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;

    if payload.action == "delete" {
        conn.interact(move |conn| {
            diesel::delete(schema::vehicles::table.filter(schema::vehicles::id.eq(&vehicle_id)))
                .execute(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    }

    Ok(Redirect::to("/vehicles"))
}

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct TestError(String);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for TestError {}

    #[test]
    fn test_internal_error_returns_500_status() {
        let (status, _) = internal_error(TestError("boom".to_string()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_internal_error_returns_error_message_as_body() {
        let (_, body) = internal_error(TestError("something went wrong".to_string()));
        assert_eq!(body, "something went wrong");
    }

    #[test]
    fn test_internal_error_with_empty_message() {
        let (status, body) = internal_error(TestError(String::new()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, "");
    }
}
