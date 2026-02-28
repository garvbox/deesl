use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, put},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::Deserialize;

use crate::AppState;
use crate::auth::extract_auth_user;
use crate::handlers::internal_error;
use crate::models::{FuelEntry, FuelStation, NewFuelEntry, NewFuelStation, NewVehicle, Vehicle};
use crate::schema::{fuel_entries, fuel_stations, vehicle_shares, vehicles};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/fuel-stations",
            get(list_fuel_stations).post(create_fuel_station),
        )
        .route("/api/fuel-stations/{id}", delete(delete_fuel_station))
        .route(
            "/api/fuel-entries",
            get(list_fuel_entries).post(create_fuel_entry),
        )
        .route(
            "/api/fuel-entries/{id}",
            get(get_fuel_entry)
                .put(update_fuel_entry)
                .delete(delete_fuel_entry),
        )
        .route("/api/vehicles", get(list_vehicles).post(create_vehicle))
        .route(
            "/api/vehicles/{id}",
            put(update_vehicle).delete(delete_vehicle),
        )
}

#[derive(serde::Serialize)]
pub struct FuelStationResponse {
    pub id: i32,
    pub name: String,
}

impl From<FuelStation> for FuelStationResponse {
    fn from(s: FuelStation) -> Self {
        Self {
            id: s.id,
            name: s.name,
        }
    }
}

pub async fn list_fuel_stations(
    State(pool): State<Pool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Verify user is authenticated and get user_id
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;
    let stations: Vec<FuelStation> = conn
        .interact(move |conn| {
            fuel_stations::table
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .order(fuel_stations::name)
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok(Json(
        stations
            .into_iter()
            .map(FuelStationResponse::from)
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize)]
pub struct CreateFuelStationRequest {
    pub name: String,
}

pub async fn create_fuel_station(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Json(payload): Json<CreateFuelStationRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Verify user is authenticated and get user_id
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let name = payload.name.clone();
    let user_id = auth_user.user_id;

    let station = conn
        .interact(move |conn| {
            diesel::insert_into(fuel_stations::table)
                .values(NewFuelStation {
                    name,
                    user_id: Some(user_id),
                })
                .returning(FuelStation::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(FuelStationResponse::from(station)),
    ))
}

pub async fn delete_fuel_station(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Verify user is authenticated and get user_id
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    // Verify ownership before deleting (allow deleting own stations or legacy stations with NULL user_id)
    let is_owner: bool = conn
        .interact(move |conn| {
            fuel_stations::table
                .filter(fuel_stations::id.eq(id))
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .first::<FuelStation>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?
        .map(|_| true)
        .unwrap_or(false);

    if !is_owner {
        return Err((
            StatusCode::FORBIDDEN,
            "You don't own this fuel station".to_string(),
        ));
    }

    conn.interact(move |conn| {
        diesel::delete(fuel_stations::table.filter(fuel_stations::id.eq(id))).execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Serialize)]
pub struct VehicleResponse {
    pub id: i32,
    pub make: String,
    pub model: String,
    pub registration: String,
    pub is_shared: bool,
    pub permission_level: Option<String>,
}

impl VehicleResponse {
    fn from_vehicle(vehicle: Vehicle, is_shared: bool, permission_level: Option<String>) -> Self {
        Self {
            id: vehicle.id,
            make: vehicle.make,
            model: vehicle.model,
            registration: vehicle.registration,
            is_shared,
            permission_level,
        }
    }
}

pub async fn list_vehicles(
    State(pool): State<Pool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    // Get owned vehicles
    let owned_vehicles: Vec<Vehicle> = conn
        .interact(move |conn| {
            vehicles::table
                .filter(vehicles::owner_id.eq(user_id))
                .order(vehicles::registration)
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    // Get shared vehicles
    let shared_vehicles: Vec<(Vehicle, String)> = conn
        .interact(move |conn| {
            vehicle_shares::table
                .inner_join(vehicles::table)
                .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                .select((Vehicle::as_select(), vehicle_shares::permission_level))
                .order(vehicles::registration)
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let mut response: Vec<VehicleResponse> = Vec::new();

    // Add owned vehicles
    for vehicle in owned_vehicles {
        response.push(VehicleResponse::from_vehicle(
            vehicle,
            false,
            Some("owner".to_string()),
        ));
    }

    // Add shared vehicles
    for (vehicle, permission_level) in shared_vehicles {
        response.push(VehicleResponse::from_vehicle(
            vehicle,
            true,
            Some(permission_level),
        ));
    }

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct CreateVehicleRequest {
    pub make: String,
    pub model: String,
    pub registration: String,
}

pub async fn create_vehicle(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Json(payload): Json<CreateVehicleRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;

    let make = payload.make.clone();
    let model = payload.model.clone();
    let registration = payload.registration.clone();
    let owner_id = auth_user.user_id;

    let vehicle: Vehicle = conn
        .interact(move |conn| {
            diesel::insert_into(vehicles::table)
                .values(NewVehicle {
                    make,
                    model,
                    registration,
                    owner_id,
                })
                .returning(Vehicle::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(VehicleResponse::from_vehicle(
            vehicle,
            false,
            Some("owner".to_string()),
        )),
    ))
}

#[derive(Deserialize)]
pub struct UpdateVehicleRequest {
    pub make: String,
    pub model: String,
    pub registration: String,
}

pub async fn update_vehicle(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i32>,
    Json(payload): Json<UpdateVehicleRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    let make = payload.make.clone();
    let model = payload.model.clone();
    let registration = payload.registration.clone();

    // Check ownership before updating
    let vehicle: Vehicle = conn
        .interact(move |conn| {
            let _vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(id))
                .filter(vehicles::owner_id.eq(user_id))
                .first(conn)
                .optional()?
                .ok_or(diesel::result::Error::NotFound)?;

            // Update the vehicle
            diesel::update(vehicles::table.filter(vehicles::id.eq(id)))
                .set((
                    vehicles::make.eq(make),
                    vehicles::model.eq(model),
                    vehicles::registration.eq(registration),
                ))
                .returning(Vehicle::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| {
            (
                StatusCode::FORBIDDEN,
                "You don't own this vehicle".to_string(),
            )
        })?;

    Ok((
        StatusCode::OK,
        Json(VehicleResponse::from_vehicle(
            vehicle,
            false,
            Some("owner".to_string()),
        )),
    ))
}

pub async fn delete_vehicle(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    // Check ownership before deleting
    let is_owner: bool = conn
        .interact(move |conn| {
            vehicles::table
                .filter(vehicles::id.eq(id))
                .filter(vehicles::owner_id.eq(user_id))
                .first::<Vehicle>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?
        .map(|_| true)
        .unwrap_or(false);

    if !is_owner {
        return Err((
            StatusCode::FORBIDDEN,
            "You don't own this vehicle".to_string(),
        ));
    }

    conn.interact(move |conn| {
        diesel::delete(vehicles::table.filter(vehicles::id.eq(id))).execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Serialize)]
pub struct FuelEntryResponse {
    pub id: i32,
    pub vehicle_id: i32,
    pub vehicle_make: String,
    pub vehicle_model: String,
    pub vehicle_registration: String,
    pub station_id: Option<i32>,
    pub station_name: Option<String>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: chrono::NaiveDateTime,
}

pub async fn list_fuel_entries(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<FuelEntryQueryParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    let entries: Vec<(FuelEntry, Option<String>, String, String, String)> = conn
        .interact(move |conn| {
            let mut query = fuel_entries::table
                .inner_join(vehicles::table)
                .left_outer_join(fuel_stations::table)
                .into_boxed();

            if let Some(vehicle_id) = params.vehicle_id {
                query = query.filter(fuel_entries::vehicle_id.eq(vehicle_id));
            }

            // Filter for owned vehicles OR vehicles shared with user
            query = query.filter(
                vehicles::owner_id.eq(user_id).or(vehicles::id.eq_any(
                    vehicle_shares::table
                        .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                        .select(vehicle_shares::vehicle_id),
                )),
            );

            query
                .select((
                    FuelEntry::as_select(),
                    fuel_stations::name.nullable(),
                    vehicles::make,
                    vehicles::model,
                    vehicles::registration,
                ))
                .order(fuel_entries::filled_at.desc())
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let response: Vec<FuelEntryResponse> = entries
        .into_iter()
        .map(
            |(e, station_name, vehicle_make, vehicle_model, vehicle_registration)| {
                FuelEntryResponse {
                    id: e.id,
                    vehicle_id: e.vehicle_id,
                    vehicle_make,
                    vehicle_model,
                    vehicle_registration,
                    station_id: e.station_id,
                    station_name,
                    mileage_km: e.mileage_km,
                    litres: e.litres,
                    cost: e.cost,
                    filled_at: e.filled_at,
                }
            },
        )
        .collect();

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct FuelEntryQueryParams {
    pub vehicle_id: Option<i32>,
}

pub async fn get_fuel_entry(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    let (entry, vehicle): (FuelEntry, Vehicle) = conn
        .interact(move |conn| {
            fuel_entries::table
                .inner_join(vehicles::table)
                .filter(fuel_entries::id.eq(id))
                .select((FuelEntry::as_select(), Vehicle::as_select()))
                .first(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "Fuel entry not found".to_string()))?;

    // Check if user has access to this vehicle
    let has_access = conn
        .interact(move |conn| {
            // Check ownership
            if vehicle.owner_id == user_id {
                return Ok::<bool, diesel::result::Error>(true);
            }
            // Check if shared
            let share_exists = vehicle_shares::table
                .filter(vehicle_shares::vehicle_id.eq(vehicle.id))
                .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                .first::<crate::models::VehicleShare>(conn)
                .optional()?;
            Ok(share_exists.is_some())
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    if !has_access {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    Ok(Json(FuelEntryResponse {
        id: entry.id,
        vehicle_id: entry.vehicle_id,
        vehicle_make: vehicle.make,
        vehicle_model: vehicle.model,
        vehicle_registration: vehicle.registration,
        station_id: entry.station_id,
        station_name: None,
        mileage_km: entry.mileage_km,
        litres: entry.litres,
        cost: entry.cost,
        filled_at: entry.filled_at,
    }))
}

#[derive(Deserialize)]
pub struct CreateFuelEntryRequest {
    pub vehicle_id: i32,
    pub station_id: Option<i32>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: Option<String>,
}

pub async fn create_fuel_entry(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Json(payload): Json<CreateFuelEntryRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    let vehicle_id = payload.vehicle_id;

    // Check if user has write access to this vehicle
    let has_write_access: (bool, bool) = conn
        .interact(move |conn| {
            let vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(vehicle_id))
                .first(conn)?;

            // Check ownership
            if vehicle.owner_id == user_id {
                return Ok::<(bool, bool), diesel::result::Error>((true, true));
            }

            // Check if shared with write permission
            let share = vehicle_shares::table
                .filter(vehicle_shares::vehicle_id.eq(vehicle_id))
                .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                .first::<crate::models::VehicleShare>(conn)
                .optional()?;

            match share {
                Some(s) => Ok((true, s.permission_level == "write")),
                None => Ok((false, false)),
            }
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "Vehicle not found".to_string()))?;

    let (has_access, has_write) = has_write_access;

    if !has_access {
        return Err((
            StatusCode::FORBIDDEN,
            "You don't have access to this vehicle".to_string(),
        ));
    }

    if !has_write {
        return Err((
            StatusCode::FORBIDDEN,
            "You don't have write permission for this vehicle".to_string(),
        ));
    }

    let station_id = payload.station_id;
    let mileage_km = payload.mileage_km;
    let litres = payload.litres;
    let cost = payload.cost;

    // Parse ISO 8601 datetime string into NaiveDateTime
    let filled_at: Option<chrono::NaiveDateTime> = match payload.filled_at {
        Some(ref dt_str) if !dt_str.is_empty() => {
            // Try to parse as DateTime<Utc> (with timezone) first, then convert to naive
            dt_str
                .parse::<chrono::DateTime<chrono::Utc>>()
                .map(|dt| dt.naive_utc())
                .or_else(|_| {
                    // Fall back to parsing as NaiveDateTime directly
                    dt_str.parse::<chrono::NaiveDateTime>()
                })
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid datetime format: {}", dt_str),
                    )
                })?
                .into()
        }
        _ => None,
    };

    let (entry, vehicle): (FuelEntry, Vehicle) = conn
        .interact(move |conn| {
            let entry = diesel::insert_into(fuel_entries::table)
                .values(NewFuelEntry {
                    vehicle_id,
                    station_id,
                    mileage_km,
                    litres,
                    cost,
                    filled_at,
                })
                .returning(FuelEntry::as_returning())
                .get_result(conn)?;

            let vehicle = vehicles::table
                .filter(vehicles::id.eq(entry.vehicle_id))
                .first(conn)?;

            Ok::<_, diesel::result::Error>((entry, vehicle))
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(FuelEntryResponse {
            id: entry.id,
            vehicle_id: entry.vehicle_id,
            vehicle_make: vehicle.make,
            vehicle_model: vehicle.model,
            vehicle_registration: vehicle.registration,
            station_id: entry.station_id,
            station_name: None,
            mileage_km: entry.mileage_km,
            litres: entry.litres,
            cost: entry.cost,
            filled_at: entry.filled_at,
        }),
    ))
}

#[derive(Deserialize)]
pub struct UpdateFuelEntryRequest {
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub station_id: Option<i32>,
}

pub async fn update_fuel_entry(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i32>,
    Json(payload): Json<UpdateFuelEntryRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    // Check if user has write access to this vehicle and update the entry
    let (entry, vehicle): (FuelEntry, Vehicle) = conn
        .interact(move |conn| {
            let entry: FuelEntry = fuel_entries::table
                .filter(fuel_entries::id.eq(id))
                .first(conn)
                .optional()?
                .ok_or(diesel::result::Error::NotFound)?;

            let vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(entry.vehicle_id))
                .first(conn)?;

            // Check if user has write access
            let has_write = if vehicle.owner_id == user_id {
                true
            } else {
                let share = vehicle_shares::table
                    .filter(vehicle_shares::vehicle_id.eq(vehicle.id))
                    .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                    .first::<crate::models::VehicleShare>(conn)
                    .optional()?;

                matches!(share, Some(s) if s.permission_level == "write")
            };

            if !has_write {
                return Err(diesel::result::Error::NotFound);
            }

            // Update the fuel entry
            let updated_entry = diesel::update(fuel_entries::table.filter(fuel_entries::id.eq(id)))
                .set((
                    fuel_entries::mileage_km.eq(payload.mileage_km),
                    fuel_entries::litres.eq(payload.litres),
                    fuel_entries::cost.eq(payload.cost),
                    fuel_entries::station_id.eq(payload.station_id),
                ))
                .returning(FuelEntry::as_returning())
                .get_result(conn)?;

            Ok((updated_entry, vehicle))
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| {
            (
                StatusCode::FORBIDDEN,
                "You don't have write permission to edit this entry".to_string(),
            )
        })?;

    Ok((
        StatusCode::OK,
        Json(FuelEntryResponse {
            id: entry.id,
            vehicle_id: entry.vehicle_id,
            vehicle_make: vehicle.make,
            vehicle_model: vehicle.model,
            vehicle_registration: vehicle.registration,
            station_id: entry.station_id,
            station_name: None,
            mileage_km: entry.mileage_km,
            litres: entry.litres,
            cost: entry.cost,
            filled_at: entry.filled_at,
        }),
    ))
}

pub async fn delete_fuel_entry(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    // Check if user has write access to this vehicle
    let has_write_access = conn
        .interact(move |conn| {
            let entry: FuelEntry = fuel_entries::table
                .filter(fuel_entries::id.eq(id))
                .first(conn)
                .map_err(|_| diesel::result::Error::NotFound)?;

            let vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(entry.vehicle_id))
                .first(conn)?;

            // Check ownership
            if vehicle.owner_id == user_id {
                return Ok::<bool, diesel::result::Error>(true);
            }

            // Check if shared with write permission
            let share = vehicle_shares::table
                .filter(vehicle_shares::vehicle_id.eq(vehicle.id))
                .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                .first::<crate::models::VehicleShare>(conn)
                .optional()?;

            match share {
                Some(s) => Ok(s.permission_level == "write"),
                None => Ok(false),
            }
        })
        .await
        .map_err(internal_error)?;

    match has_write_access {
        Ok(false) => {
            return Err((
                StatusCode::FORBIDDEN,
                "You don't have write permission to delete this entry".to_string(),
            ));
        }
        Err(_) => return Err((StatusCode::NOT_FOUND, "Fuel entry not found".to_string())),
        Ok(true) => {}
    }

    conn.interact(move |conn| {
        diesel::delete(fuel_entries::table.filter(fuel_entries::id.eq(id))).execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FuelStation, Vehicle};
    use chrono::NaiveDateTime;

    fn epoch() -> NaiveDateTime {
        chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()
    }

    fn make_vehicle(id: i32, make: &str, model: &str, registration: &str) -> Vehicle {
        Vehicle {
            id,
            make: make.to_string(),
            model: model.to_string(),
            registration: registration.to_string(),
            created: epoch(),
            owner_id: 1,
        }
    }

    fn make_station(id: i32, name: &str) -> FuelStation {
        FuelStation {
            id,
            name: name.to_string(),
            created_at: epoch(),
            user_id: Some(1),
        }
    }

    // --- FuelStationResponse::from ---

    #[test]
    fn test_fuel_station_response_maps_id_and_name() {
        let station = make_station(5, "Shell Grafton Street");
        let response = FuelStationResponse::from(station);
        assert_eq!(response.id, 5);
        assert_eq!(response.name, "Shell Grafton Street");
    }

    #[test]
    fn test_fuel_station_response_does_not_include_created_at() {
        // Compile-time guarantee: FuelStationResponse only has id + name
        let station = make_station(1, "BP");
        let response = FuelStationResponse::from(station);
        let _ = response.id;
        let _ = response.name;
    }

    // --- VehicleResponse::from_vehicle ---

    #[test]
    fn test_vehicle_response_maps_all_fields() {
        let vehicle = make_vehicle(3, "Toyota", "Corolla", "241-D-12345");
        let response = VehicleResponse::from_vehicle(vehicle, false, Some("owner".to_string()));
        assert_eq!(response.id, 3);
        assert_eq!(response.make, "Toyota");
        assert_eq!(response.model, "Corolla");
        assert_eq!(response.registration, "241-D-12345");
        assert!(!response.is_shared);
        assert_eq!(response.permission_level, Some("owner".to_string()));
    }

    #[test]
    fn test_vehicle_response_from_shared_vehicle() {
        let vehicle = make_vehicle(5, "Ford", "Focus", "222-G-9999");
        let response = VehicleResponse::from_vehicle(vehicle, true, Some("write".to_string()));
        assert!(response.is_shared);
        assert_eq!(response.permission_level, Some("write".to_string()));
    }
}
