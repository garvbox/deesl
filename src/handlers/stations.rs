use askama::Template;
use axum::{
    Form, Router,
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use diesel::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use super::{HxRedirect, fuzzy_match, internal_error};
use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::db::DbConn;
use crate::models::{FuelStation, NewFuelStation};
use crate::schema::fuel_stations;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(stations_page).post(create_station))
        .route("/{id}", post(update_station).delete(delete_station))
        .route("/{id}/merge", post(merge_stations))
        .route("/htmx/search", get(htmx_station_search))
}

#[derive(Template)]
#[template(path = "stations.html")]
pub struct StationsTemplate {
    pub logged_in: bool,
    pub stations: Vec<FuelStation>,
}

pub async fn stations_page(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let stations: Vec<FuelStation> = conn
        .interact(move |conn| {
            fuel_stations::table
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .order(fuel_stations::name)
                .load::<FuelStation>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = StationsTemplate {
        logged_in: true,
        stations,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Deserialize)]
pub struct CreateStationForm {
    pub name: String,
}

pub async fn create_station(
    DbConn(conn): DbConn,
    user: AuthUser,
    Form(payload): Form<CreateStationForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    conn.interact(move |conn| {
        diesel::insert_into(fuel_stations::table)
            .values(NewFuelStation {
                name: payload.name,
                user_id: Some(user_id),
            })
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(HxRedirect("/stations"))
}

#[derive(Deserialize)]
pub struct UpdateStationForm {
    pub name: String,
}

pub async fn update_station(
    DbConn(conn): DbConn,
    user: AuthUser,
    axum::extract::Path(station_id): axum::extract::Path<i32>,
    Form(payload): Form<UpdateStationForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let is_owner: bool = conn
        .interact(move |conn| {
            fuel_stations::table
                .filter(fuel_stations::id.eq(station_id))
                .filter(fuel_stations::user_id.eq(user_id))
                .select(fuel_stations::id)
                .first::<i32>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some();

    if !is_owner {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    conn.interact(move |conn| {
        diesel::update(fuel_stations::table.filter(fuel_stations::id.eq(station_id)))
            .set(crate::models::UpdateFuelStation {
                name: Some(payload.name),
            })
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(HxRedirect("/stations"))
}

#[derive(Deserialize)]
pub struct MergeStationsForm {
    pub target_id: i32,
}

pub async fn merge_stations(
    DbConn(conn): DbConn,
    user: AuthUser,
    axum::extract::Path(source_id): axum::extract::Path<i32>,
    Form(payload): Form<MergeStationsForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;
    let target_id = payload.target_id;

    if source_id == target_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "Cannot merge station into itself".to_string(),
        ));
    }

    conn.interact(move |conn| {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Verify source station ownership
            let source_exists = fuel_stations::table
                .filter(fuel_stations::id.eq(source_id))
                .filter(fuel_stations::user_id.eq(user_id))
                .execute(conn)?
                > 0;

            if !source_exists {
                return Err(diesel::result::Error::NotFound);
            }

            // Verify target station exists and is accessible
            let target_exists = fuel_stations::table
                .filter(fuel_stations::id.eq(target_id))
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .execute(conn)?
                > 0;

            if !target_exists {
                return Err(diesel::result::Error::NotFound);
            }

            // Update fuel entries
            diesel::update(
                crate::schema::fuel_entries::table
                    .filter(crate::schema::fuel_entries::station_id.eq(source_id)),
            )
            .set(crate::schema::fuel_entries::station_id.eq(target_id))
            .execute(conn)?;

            // Delete source station
            diesel::delete(fuel_stations::table.filter(fuel_stations::id.eq(source_id)))
                .execute(conn)?;

            Ok(())
        })
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(HxRedirect("/stations"))
}

pub async fn delete_station(
    DbConn(conn): DbConn,
    user: AuthUser,
    axum::extract::Path(station_id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let is_owner: bool = conn
        .interact(move |conn| {
            fuel_stations::table
                .filter(fuel_stations::id.eq(station_id))
                .filter(fuel_stations::user_id.eq(user_id))
                .select(fuel_stations::id)
                .first::<i32>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some();

    if !is_owner {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    conn.interact(move |conn| {
        diesel::delete(fuel_stations::table.filter(fuel_stations::id.eq(station_id))).execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Html(""))
}

#[derive(Template)]
#[template(path = "fragments/station_search_results.html")]
pub struct StationSearchResultsTemplate {
    pub stations: Vec<FuelStation>,
    pub query: String,
}

pub async fn htmx_station_search(
    DbConn(conn): DbConn,
    user: AuthUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let query = params.get("q").cloned().unwrap_or_default();
    let query_lower = query.to_lowercase();

    if query.len() < 2 {
        return Ok(Html(String::new()));
    }

    let user_id = user.user_id;

    // Fetch all stations for the user (user's own + global)
    let stations: Vec<FuelStation> = conn
        .interact(move |conn| {
            fuel_stations::table
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .order(fuel_stations::name)
                .load::<FuelStation>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Filter stations by fuzzy matching
    let filtered_stations: Vec<FuelStation> = stations
        .into_iter()
        .filter(|s| {
            let name_lower = s.name.to_lowercase();
            // Check if query is a substring (case-insensitive)
            name_lower.contains(&query_lower) ||
            // Or if all query characters appear in order
            fuzzy_match(&name_lower, &query_lower)
        })
        .take(10) // Limit to 10 results
        .collect();

    let template = StationSearchResultsTemplate {
        stations: filtered_stations,
        query,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}
