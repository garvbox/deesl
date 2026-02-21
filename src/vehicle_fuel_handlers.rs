use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::Deserialize;

use crate::handlers::internal_error;
use crate::models::{FuelEntry, FuelStation, NewFuelEntry, NewFuelStation, NewVehicle, Vehicle};
use crate::schema::{fuel_entries, fuel_stations, vehicles};

pub fn router() -> Router<Pool> {
    Router::new()
        .route("/api/fuel-stations", get(list_fuel_stations).post(create_fuel_station))
        .route("/api/fuel-stations/{id}", delete(delete_fuel_station))
        .route("/api/fuel-entries", get(list_fuel_entries).post(create_fuel_entry))
        .route("/api/fuel-entries/{id}", get(get_fuel_entry).delete(delete_fuel_entry))
        .route("/api/vehicles", get(list_vehicles).post(create_vehicle))
}

#[derive(serde::Serialize)]
pub struct FuelStationResponse {
    pub id: i32,
    pub name: String,
}

impl From<FuelStation> for FuelStationResponse {
    fn from(s: FuelStation) -> Self {
        Self { id: s.id, name: s.name }
    }
}

pub async fn list_fuel_stations(
    State(pool): State<Pool>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let stations: Vec<FuelStation> = conn
        .interact(|conn| {
            fuel_stations::table
                .order(fuel_stations::name)
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(stations.into_iter().map(FuelStationResponse::from).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
pub struct CreateFuelStationRequest {
    pub name: String,
}

pub async fn create_fuel_station(
    State(pool): State<Pool>,
    Json(payload): Json<CreateFuelStationRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let name = payload.name.clone();
    
    let station = conn
        .interact(move |conn| {
            diesel::insert_into(fuel_stations::table)
                .values(NewFuelStation { name })
                .returning(FuelStation::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok((StatusCode::CREATED, Json(FuelStationResponse::from(station))))
}

pub async fn delete_fuel_station(
    State(pool): State<Pool>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    conn.interact(move |conn| {
        diesel::delete(fuel_stations::table.filter(fuel_stations::id.eq(id)))
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Serialize)]
pub struct VehicleResponse {
    pub id: i32,
    pub make: String,
    pub model: String,
    pub registration: String,
}

impl From<Vehicle> for VehicleResponse {
    fn from(v: Vehicle) -> Self {
        Self {
            id: v.id,
            make: v.make,
            model: v.model,
            registration: v.registration,
        }
    }
}

pub async fn list_vehicles(
    State(pool): State<Pool>,
    axum::extract::Query(params): axum::extract::Query<VehicleQueryParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    let vehicles: Vec<Vehicle> = conn
        .interact(move |conn| {
            let mut query = vehicles::table.into_boxed();
            if let Some(user_id) = params.user_id {
                query = query.filter(vehicles::owner_id.eq(user_id));
            }
            query.load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(vehicles.into_iter().map(VehicleResponse::from).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
pub struct VehicleQueryParams {
    pub user_id: Option<i32>,
}

#[derive(Deserialize)]
pub struct CreateVehicleRequest {
    pub make: String,
    pub model: String,
    pub registration: String,
    pub owner_id: i32,
}

pub async fn create_vehicle(
    State(pool): State<Pool>,
    Json(payload): Json<CreateVehicleRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    let make = payload.make.clone();
    let model = payload.model.clone();
    let registration = payload.registration.clone();
    let owner_id = payload.owner_id;
    
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok((StatusCode::CREATED, Json(VehicleResponse::from(vehicle))))
}

#[derive(serde::Serialize)]
pub struct FuelEntryResponse {
    pub id: i32,
    pub vehicle_id: i32,
    pub station_id: Option<i32>,
    pub station_name: Option<String>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: chrono::NaiveDateTime,
}

pub async fn list_fuel_entries(
    State(pool): State<Pool>,
    axum::extract::Query(params): axum::extract::Query<FuelEntryQueryParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    let entries: Vec<(FuelEntry, Option<String>)> = conn
        .interact(move |conn| {
            let mut query = fuel_entries::table
                .left_outer_join(fuel_stations::table)
                .into_boxed();
            
            if let Some(vehicle_id) = params.vehicle_id {
                query = query.filter(fuel_entries::vehicle_id.eq(vehicle_id));
            }
            
            query
                .select((FuelEntry::as_select(), fuel_stations::name.nullable()))
                .order(fuel_entries::filled_at.desc())
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    let response: Vec<FuelEntryResponse> = entries
        .into_iter()
        .map(|(e, station_name)| FuelEntryResponse {
            id: e.id,
            vehicle_id: e.vehicle_id,
            station_id: e.station_id,
            station_name,
            mileage_km: e.mileage_km,
            litres: e.litres,
            cost: e.cost,
            filled_at: e.filled_at,
        })
        .collect();

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct FuelEntryQueryParams {
    pub vehicle_id: Option<i32>,
}

pub async fn get_fuel_entry(
    State(pool): State<Pool>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    let entry: FuelEntry = conn
        .interact(move |conn| {
            fuel_entries::table
                .filter(fuel_entries::id.eq(id))
                .first(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "Fuel entry not found".to_string()))?;

    Ok(Json(FuelEntryResponse {
        id: entry.id,
        vehicle_id: entry.vehicle_id,
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
    pub filled_at: Option<chrono::NaiveDateTime>,
}

pub async fn create_fuel_entry(
    State(pool): State<Pool>,
    Json(payload): Json<CreateFuelEntryRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    let vehicle_id = payload.vehicle_id;
    let station_id = payload.station_id;
    let mileage_km = payload.mileage_km;
    let litres = payload.litres;
    let cost = payload.cost;
    let filled_at = payload.filled_at;
    
    let entry: FuelEntry = conn
        .interact(move |conn| {
            diesel::insert_into(fuel_entries::table)
                .values(NewFuelEntry {
                    vehicle_id,
                    station_id,
                    mileage_km,
                    litres,
                    cost,
                    filled_at,
                })
                .returning(FuelEntry::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok((StatusCode::CREATED, Json(FuelEntryResponse {
        id: entry.id,
        vehicle_id: entry.vehicle_id,
        station_id: entry.station_id,
        station_name: None,
        mileage_km: entry.mileage_km,
        litres: entry.litres,
        cost: entry.cost,
        filled_at: entry.filled_at,
    })))
}

pub async fn delete_fuel_entry(
    State(pool): State<Pool>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    
    conn.interact(move |conn| {
        diesel::delete(fuel_entries::table.filter(fuel_entries::id.eq(id)))
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
