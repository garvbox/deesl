use askama::Template;
use axum::{
    Form, Router,
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, post},
};
use diesel::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use super::{HxRedirect, check_vehicle_write_access, internal_error};
use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::db::DbConn;
use crate::models::{FuelEntry, FuelStation, User, Vehicle};
use crate::schema::{fuel_stations, users, vehicles};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(fuel_entries_page).post(create_fuel_entry))
        .route("/new", get(new_fuel_entry))
        .route("/{id}/edit", get(edit_fuel_entry))
        .route("/{id}", post(update_fuel_entry))
        .route("/htmx/recent", get(htmx_recent_entries))
        .route("/htmx/{id}", delete(htmx_delete_fuel_entry))
}

#[derive(Template)]
#[template(path = "add_fuel_entry.html")]
pub struct AddFuelEntryTemplate {
    pub logged_in: bool,
    pub vehicles: Vec<Vehicle>,
    pub stations: Vec<FuelStation>,
    pub current_time: String,
    pub distance_unit: String,
    pub volume_unit: String,
}

pub async fn new_fuel_entry(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let (user_vehicles, user_stations, db_user): (Vec<Vehicle>, Vec<FuelStation>, User) = conn
        .interact(move |conn| {
            let vehicles = vehicles::table
                .filter(vehicles::owner_id.eq(user_id))
                .order(vehicles::registration)
                .load::<Vehicle>(conn)?;
            let stations = fuel_stations::table
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .order(fuel_stations::name)
                .load::<FuelStation>(conn)?;
            let db_user = users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)?;
            Ok::<(Vec<Vehicle>, Vec<FuelStation>, User), diesel::result::Error>((
                vehicles, stations, db_user,
            ))
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = AddFuelEntryTemplate {
        logged_in: true,
        vehicles: user_vehicles,
        stations: user_stations,
        current_time: chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string(),
        distance_unit: db_user.distance_unit,
        volume_unit: db_user.volume_unit,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Deserialize)]
pub struct CreateFuelEntryForm {
    pub vehicle_id: i32,
    pub station_id: i32,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: Option<String>,
}

pub async fn create_fuel_entry(
    DbConn(conn): DbConn,
    user: AuthUser,
    Form(payload): Form<CreateFuelEntryForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let vehicle_id = payload.vehicle_id;

    check_vehicle_write_access(&conn, user.user_id, vehicle_id).await?;

    let filled_at = payload
        .filled_at
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M").ok());

    conn.interact(move |conn| {
        diesel::insert_into(crate::schema::fuel_entries::table)
            .values(crate::models::NewFuelEntry {
                vehicle_id,
                station_id: Some(payload.station_id),
                mileage_km: payload.mileage_km,
                litres: payload.litres,
                cost: payload.cost,
                filled_at,
            })
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(HxRedirect("/dashboard"))
}

pub async fn htmx_delete_fuel_entry(
    DbConn(conn): DbConn,
    user: AuthUser,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    // First check if entry exists and get vehicle_id
    let vehicle_id: i32 = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .filter(crate::schema::fuel_entries::id.eq(id))
                .select(crate::schema::fuel_entries::vehicle_id)
                .first::<i32>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Entry not found".to_string()))?;

    check_vehicle_write_access(&conn, user_id, vehicle_id).await?;

    // Delete the entry
    conn.interact(move |conn| {
        diesel::delete(
            crate::schema::fuel_entries::table.filter(crate::schema::fuel_entries::id.eq(id)),
        )
        .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Html(""))
}

#[derive(Template)]
#[template(path = "fragments/recent_entries.html")]
pub struct RecentEntriesTemplate {
    pub entries: Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>,
    pub distance_unit: String,
    pub volume_unit: String,
}

impl RecentEntriesTemplate {
    pub fn format_float(&self, val: &f64) -> String {
        format!("{:.2}", val)
    }
}

pub async fn htmx_recent_entries(
    DbConn(conn): DbConn,
    user: AuthUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let (entries, db_user): (
        Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>,
        User,
    ) = conn
        .interact(move |conn| {
            let entries = crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .left_join(crate::schema::fuel_stations::table)
                .filter(vehicles::owner_id.eq(user_id))
                .order(crate::schema::fuel_entries::filled_at.desc())
                .limit(5)
                .select((
                    crate::models::FuelEntry::as_select(),
                    Vehicle::as_select(),
                    Option::<FuelStation>::as_select(),
                ))
                .load::<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>(conn)?;
            let db_user = users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)?;
            Ok::<
                (
                    Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>,
                    User,
                ),
                diesel::result::Error,
            >((entries, db_user))
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = RecentEntriesTemplate {
        entries,
        distance_unit: db_user.distance_unit,
        volume_unit: db_user.volume_unit,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Template)]
#[template(path = "edit_fuel_entry.html")]
pub struct EditFuelEntryTemplate {
    pub logged_in: bool,
    pub entry: FuelEntry,
    pub vehicle: Vehicle,
    pub stations: Vec<FuelStation>,
    pub current_station_name: String,
    pub filled_at_formatted: String,
    pub distance_unit: String,
    pub volume_unit: String,
}

pub async fn edit_fuel_entry(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
    axum::extract::Path(entry_id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let (result, stations, db_user): (Option<(FuelEntry, Vehicle)>, Vec<FuelStation>, User) = conn
        .interact(move |conn| {
            let entry_result = crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(crate::schema::fuel_entries::id.eq(entry_id))
                .select((FuelEntry::as_select(), Vehicle::as_select()))
                .first::<(FuelEntry, Vehicle)>(conn)
                .optional()?;
            let stations = fuel_stations::table
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .order(fuel_stations::name)
                .load::<FuelStation>(conn)?;
            let db_user = users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)?;
            Ok::<(Option<(FuelEntry, Vehicle)>, Vec<FuelStation>, User), diesel::result::Error>((
                entry_result,
                stations,
                db_user,
            ))
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (entry, vehicle) = result.ok_or((StatusCode::NOT_FOUND, "Entry not found".to_string()))?;

    check_vehicle_write_access(&conn, user_id, vehicle.id).await?;

    let filled_at_formatted = entry.filled_at.format("%Y-%m-%dT%H:%M").to_string();

    // Find the current station name
    let current_station_name = entry
        .station_id
        .and_then(|id| stations.iter().find(|s| s.id == id))
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let template = EditFuelEntryTemplate {
        logged_in: true,
        entry,
        vehicle,
        stations,
        current_station_name,
        filled_at_formatted,
        distance_unit: db_user.distance_unit,
        volume_unit: db_user.volume_unit,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Deserialize)]
pub struct UpdateFuelEntryForm {
    pub station_id: i32,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: String,
}

pub async fn update_fuel_entry(
    DbConn(conn): DbConn,
    user: AuthUser,
    axum::extract::Path(entry_id): axum::extract::Path<i32>,
    Form(payload): Form<UpdateFuelEntryForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let vehicle_id: i32 = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .filter(crate::schema::fuel_entries::id.eq(entry_id))
                .select(crate::schema::fuel_entries::vehicle_id)
                .first::<i32>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Entry not found".to_string()))?;

    check_vehicle_write_access(&conn, user_id, vehicle_id).await?;

    let filled_at = chrono::NaiveDateTime::parse_from_str(&payload.filled_at, "%Y-%m-%dT%H:%M")
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date format".to_string()))?;

    conn.interact(move |conn| {
        diesel::update(
            crate::schema::fuel_entries::table.filter(crate::schema::fuel_entries::id.eq(entry_id)),
        )
        .set(crate::models::UpdateFuelEntry {
            station_id: Some(payload.station_id),
            mileage_km: Some(payload.mileage_km),
            litres: Some(payload.litres),
            cost: Some(payload.cost),
            filled_at: Some(filled_at),
        })
        .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(HxRedirect("/dashboard"))
}

#[derive(Template)]
#[template(path = "fuel_entries.html")]
pub struct FuelEntriesTemplate {
    pub logged_in: bool,
    pub entries: Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>,
    pub vehicles: Vec<Vehicle>,
    pub stations: Vec<FuelStation>,
    pub filter_vehicle_id: Option<i32>,
    pub filter_station_id: Option<i32>,
    pub filter_date_from: String,
    pub filter_date_to: String,
    pub total_entries: usize,
    pub total_litres: f64,
    pub total_cost: f64,
    pub distance_unit: String,
    pub volume_unit: String,
}

pub async fn fuel_entries_page(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    // Parse filter parameters
    let filter_vehicle_id = params.get("vehicle_id").and_then(|v| v.parse::<i32>().ok());
    let filter_station_id = params.get("station_id").and_then(|v| v.parse::<i32>().ok());
    let filter_date_from = params.get("date_from").cloned().unwrap_or_default();
    let filter_date_to = params.get("date_to").cloned().unwrap_or_default();

    // Fetch all vehicles for the user (for filter dropdown)
    let vehicles: Vec<Vehicle> = conn
        .interact(move |conn| {
            vehicles::table
                .filter(vehicles::owner_id.eq(user_id))
                .order(vehicles::registration)
                .load::<Vehicle>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch all stations for the user (for filter dropdown)
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

    // Fetch user for unit preferences
    let db_user: User = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clone date filters for the closure
    let filter_date_from_clone = filter_date_from.clone();
    let filter_date_to_clone = filter_date_to.clone();

    // Fetch fuel entries with filters
    let entries: Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)> = conn
        .interact(move |conn| {
            let mut query = crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .left_join(crate::schema::fuel_stations::table)
                .filter(vehicles::owner_id.eq(user_id))
                .into_boxed();

            // Apply vehicle filter
            if let Some(vid) = filter_vehicle_id {
                query = query.filter(crate::schema::fuel_entries::vehicle_id.eq(vid));
            }

            // Apply station filter
            if let Some(sid) = filter_station_id {
                query = query.filter(crate::schema::fuel_entries::station_id.eq(sid));
            }

            // Apply date filters
            if !filter_date_from_clone.is_empty()
                && let Ok(date) =
                    chrono::NaiveDate::parse_from_str(&filter_date_from_clone, "%Y-%m-%d")
            {
                let datetime = date.and_hms_opt(0, 0, 0).unwrap();
                query = query.filter(crate::schema::fuel_entries::filled_at.ge(datetime));
            }

            if !filter_date_to_clone.is_empty()
                && let Ok(date) =
                    chrono::NaiveDate::parse_from_str(&filter_date_to_clone, "%Y-%m-%d")
            {
                let datetime = date.and_hms_opt(23, 59, 59).unwrap();
                query = query.filter(crate::schema::fuel_entries::filled_at.le(datetime));
            }

            query
                .order(crate::schema::fuel_entries::filled_at.desc())
                .select((
                    crate::models::FuelEntry::as_select(),
                    Vehicle::as_select(),
                    Option::<FuelStation>::as_select(),
                ))
                .load::<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Calculate totals
    let total_entries = entries.len();
    let total_litres: f64 = entries.iter().map(|(e, _, _)| e.litres).sum();
    let total_cost: f64 = entries.iter().map(|(e, _, _)| e.cost).sum();

    let template = FuelEntriesTemplate {
        logged_in: true,
        entries,
        vehicles,
        stations,
        filter_vehicle_id,
        filter_station_id,
        filter_date_from,
        filter_date_to,
        total_entries,
        total_litres,
        total_cost,
        distance_unit: db_user.distance_unit,
        volume_unit: db_user.volume_unit,
    };

    Ok(Html(template.render().map_err(internal_error)?))
}
