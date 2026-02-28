use axum::{
    Json, Router,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use chrono::Datelike;
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::AppState;
use crate::auth::extract_auth_user;
use crate::handlers::internal_error;
use crate::models::{FuelEntry, FuelStation, NewFuelEntry, NewFuelStation, Vehicle};
use crate::schema::{fuel_entries, fuel_stations, vehicles};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/fuel-entries/import/preview", post(preview_import))
        .route("/api/fuel-entries/import", post(execute_import))
}

#[derive(Serialize)]
pub struct PreviewResponse {
    columns: Vec<String>,
    preview: Vec<Vec<String>>,
    suggested_mappings: HashMap<String, String>,
    total_rows: usize,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct ExecuteImportRequest {
    vehicle_id: i32,
    mappings: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ImportResult {
    imported: usize,
    skipped: usize,
    stations_created: usize,
    errors: Vec<String>,
}

pub async fn preview_import(
    State(pool): State<Pool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let _conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    let mut csv_data: Option<Vec<u8>> = None;
    let mut vehicle_id: Option<i32> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        let data = field.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read field bytes: {}", e),
            )
        })?;

        match name.as_str() {
            "file" => csv_data = Some(data.to_vec()),
            "vehicle_id" => {
                vehicle_id =
                    Some(String::from_utf8_lossy(&data).trim().parse().map_err(|_| {
                        (StatusCode::BAD_REQUEST, "Invalid vehicle_id".to_string())
                    })?);
            }
            _ => {}
        }
    }

    let vehicle_id =
        vehicle_id.ok_or((StatusCode::BAD_REQUEST, "vehicle_id required".to_string()))?;
    let csv_data = csv_data.ok_or((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))?;

    let conn = pool.get().await.map_err(internal_error)?;
    let has_access = conn
        .interact(move |conn| {
            let vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(vehicle_id))
                .first(conn)
                .optional()?
                .ok_or(diesel::result::Error::NotFound)?;

            if vehicle.owner_id == user_id {
                return Ok::<bool, diesel::result::Error>(true);
            }
            Err(diesel::result::Error::NotFound)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::FORBIDDEN, "Access denied".to_string()))?;

    if !has_access {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    let mut reader = csv::Reader::from_reader(&csv_data[..]);
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("CSV parse error: {}", e)))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut preview: Vec<Vec<String>> = Vec::new();
    for result in reader.records() {
        let record =
            result.map_err(|e| (StatusCode::BAD_REQUEST, format!("CSV parse error: {}", e)))?;
        preview.push(record.iter().map(|s| s.to_string()).collect());
        if preview.len() >= 5 {
            break;
        }
    }

    let mut suggested_mappings: HashMap<String, String> = HashMap::new();
    for header in &headers {
        let lower = header.to_lowercase();
        let mapping = if lower.contains("date") {
            "filled_at_date"
        } else if lower.contains("time") {
            "filled_at_time"
        } else if lower.contains("location") || lower.contains("station") {
            "station"
        } else if lower.contains("litre") || lower == "litres" {
            "litres"
        } else if lower.contains("cost") && !lower.contains("litre") && !lower.contains("/") {
            "cost"
        } else if lower.contains("km") || lower.contains("mileage") {
            "mileage_km"
        } else {
            ""
        };
        if !mapping.is_empty() {
            suggested_mappings.insert(header.clone(), mapping.to_string());
        }
    }

    let total_rows = preview.len();

    Ok(Json(PreviewResponse {
        columns: headers,
        preview,
        suggested_mappings,
        total_rows,
    }))
}

pub async fn execute_import(
    State(pool): State<Pool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let user_id = auth_user.user_id;

    let mut csv_data: Option<Vec<u8>> = None;
    let mut mappings: Option<HashMap<String, String>> = None;
    let mut vehicle_id: Option<i32> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        let data = field.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read field bytes: {}", e),
            )
        })?;

        match name.as_str() {
            "file" => csv_data = Some(data.to_vec()),
            "vehicle_id" => {
                vehicle_id =
                    Some(String::from_utf8_lossy(&data).trim().parse().map_err(|_| {
                        (StatusCode::BAD_REQUEST, "Invalid vehicle_id".to_string())
                    })?);
            }
            "mappings" => {
                mappings = Some(
                    serde_json::from_str(&String::from_utf8_lossy(&data)).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            format!("Invalid mappings JSON: {}", e),
                        )
                    })?,
                );
            }
            _ => {}
        }
    }

    let vehicle_id =
        vehicle_id.ok_or((StatusCode::BAD_REQUEST, "vehicle_id required".to_string()))?;
    let csv_data = csv_data.ok_or((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))?;
    let mappings = mappings.ok_or((StatusCode::BAD_REQUEST, "mappings required".to_string()))?;

    let conn = pool.get().await.map_err(internal_error)?;

    let _has_access = conn
        .interact(move |conn| {
            let vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(vehicle_id))
                .first(conn)
                .optional()?
                .ok_or(diesel::result::Error::NotFound)?;

            if vehicle.owner_id == user_id {
                return Ok::<bool, diesel::result::Error>(true);
            }
            Err(diesel::result::Error::NotFound)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::FORBIDDEN, "Access denied".to_string()))?;

    let existing_stations: HashMap<String, i32> = pool
        .get()
        .await
        .map_err(internal_error)?
        .interact(move |conn| {
            fuel_stations::table
                .filter(
                    fuel_stations::user_id
                        .eq(user_id)
                        .or(fuel_stations::user_id.is_null()),
                )
                .load::<FuelStation>(conn)
                .map(|stations| {
                    stations
                        .into_iter()
                        .map(|s| (normalize_station_name(&s.name), s.id))
                        .collect::<HashMap<_, _>>()
                })
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let mut reader = csv::Reader::from_reader(&csv_data[..]);
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("CSV parse error: {}", e)))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut stations_created = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let stations_map = existing_stations;
    let mut new_stations_cache: HashMap<String, i32> = HashMap::new();
    let date_col = mappings.get("filled_at_date").cloned();
    let time_col = mappings.get("filled_at_time").cloned();
    let station_col = mappings.get("station").cloned();
    let litres_col = mappings.get("litres").cloned();
    let cost_col = mappings.get("cost").cloned();
    let km_col = mappings.get("mileage_km").cloned();

    for (row_num, result) in reader.records().enumerate() {
        let row_num = row_num + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Row {}: CSV parse error - {}", row_num, e));
                continue;
            }
        };

        let row_data: HashMap<String, String> = headers
            .iter()
            .zip(record.iter())
            .map(|(h, v)| (h.clone(), v.to_string()))
            .collect();

        if row_data.values().all(|v| v.trim().is_empty()) {
            continue;
        }

        let date_str = date_col.as_ref().and_then(|col| row_data.get(col).cloned());
        let time_str = time_col.as_ref().and_then(|col| row_data.get(col).cloned());
        let station_name = station_col
            .as_ref()
            .and_then(|col| row_data.get(col).cloned());
        let litres_str = litres_col
            .as_ref()
            .and_then(|col| row_data.get(col).cloned());
        let cost_str = cost_col.as_ref().and_then(|col| row_data.get(col).cloned());
        let km_str = km_col.as_ref().and_then(|col| row_data.get(col).cloned());

        if date_str.is_none() || litres_str.is_none() || cost_str.is_none() || km_str.is_none() {
            errors.push(format!(
                "Row {}: Missing required fields (date, litres, cost, or mileage)",
                row_num
            ));
            continue;
        }

        let filled_at = match parse_datetime(date_str.as_deref().unwrap(), time_str.as_deref()) {
            Some(dt) => dt,
            None => {
                errors.push(format!(
                    "Row {}: Invalid date/time format: {:?} {:?}",
                    row_num,
                    date_str.as_deref().unwrap_or("none"),
                    time_str.as_deref().unwrap_or("none")
                ));
                continue;
            }
        };

        let litres = match parse_float(litres_str.as_deref().unwrap()) {
            Some(v) => v,
            None => {
                errors.push(format!(
                    "Row {}: Invalid litres value: {}",
                    row_num,
                    litres_str.as_deref().unwrap_or("")
                ));
                continue;
            }
        };

        let cost = match parse_float(cost_str.as_deref().unwrap()) {
            Some(v) => v,
            None => {
                errors.push(format!(
                    "Row {}: Invalid cost value: {}",
                    row_num,
                    cost_str.as_deref().unwrap_or("")
                ));
                continue;
            }
        };

        let mileage_km = match parse_int(km_str.as_deref().unwrap()) {
            Some(v) => v,
            None => {
                errors.push(format!(
                    "Row {}: Invalid mileage value: {}",
                    row_num,
                    km_str.as_deref().unwrap_or("")
                ));
                continue;
            }
        };

        let station_id = if let Some(name) = station_name {
            if name.trim().is_empty() {
                None
            } else {
                let normalized = normalize_station_name(&name);
                if let Some(&id) = new_stations_cache.get(&normalized) {
                    Some(id)
                } else if let Some(&id) = stations_map.get(&normalized) {
                    Some(id)
                } else {
                    let new_station_name = name.trim().to_string();
                    let new_id_result = pool
                        .get()
                        .await
                        .map_err(internal_error)?
                        .interact(move |conn| {
                            diesel::insert_into(fuel_stations::table)
                                .values(NewFuelStation {
                                    name: new_station_name,
                                    user_id: Some(user_id),
                                })
                                .returning(fuel_stations::id)
                                .get_result::<i32>(conn)
                        })
                        .await
                        .map_err(internal_error);

                    match new_id_result {
                        Ok(Ok(id)) => {
                            new_stations_cache.insert(normalized.clone(), id);
                            stations_created += 1;
                            Some(id)
                        }
                        Ok(Err(diesel::result::Error::DatabaseError(
                            diesel::result::DatabaseErrorKind::UniqueViolation,
                            _,
                        ))) => {
                            let existing_result = pool
                                .get()
                                .await
                                .map_err(internal_error)?
                                .interact(move |conn| {
                                    fuel_stations::table
                                        .filter(fuel_stations::name.eq(name.trim()))
                                        .filter(
                                            fuel_stations::user_id
                                                .eq(user_id)
                                                .or(fuel_stations::user_id.is_null()),
                                        )
                                        .select(fuel_stations::id)
                                        .first::<i32>(conn)
                                })
                                .await
                                .map_err(internal_error);
                            match existing_result {
                                Ok(Ok(id)) => {
                                    new_stations_cache.insert(normalized.clone(), id);
                                    Some(id)
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
            }
        } else {
            None
        };

        let result = pool
            .get()
            .await
            .map_err(internal_error)?
            .interact(move |conn| {
                diesel::insert_into(fuel_entries::table)
                    .values(NewFuelEntry {
                        vehicle_id,
                        station_id,
                        mileage_km,
                        litres,
                        cost,
                        filled_at: Some(filled_at),
                    })
                    .returning(FuelEntry::as_returning())
                    .get_result(conn)
            })
            .await
            .map_err(internal_error);

        match result {
            Ok(Ok(_)) => imported += 1,
            Ok(Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ))) => skipped += 1,
            Ok(Err(e)) => {
                errors.push(format!("Row {}: Database error - {}", row_num, e));
            }
            Err(e) => {
                errors.push(format!("Row {}: Internal error - {}", row_num, e.1));
            }
        }
    }

    Ok(Json(ImportResult {
        imported,
        skipped,
        stations_created,
        errors: errors.into_iter().take(10).collect(),
    }))
}

fn normalize_station_name(name: &str) -> String {
    name.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_datetime(date_str: &str, time_str: Option<&str>) -> Option<chrono::NaiveDateTime> {
    let date = parse_date(date_str)?;
    let time = time_str
        .and_then(parse_time)
        .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    Some(chrono::NaiveDateTime::new(date, time))
}

fn parse_date(date_str: &str) -> Option<chrono::NaiveDate> {
    let trimmed = date_str.trim();

    if let Ok(dt) = chrono::NaiveDate::parse_from_str(trimmed, "%d/%m/%Y") {
        return Some(dt);
    }

    if let Ok(dt) = chrono::NaiveDate::parse_from_str(trimmed, "%d/%m/%y") {
        let year = dt.year();
        let full_year = if year < 50 { 2000 + year } else { 1900 + year };
        return chrono::NaiveDate::from_ymd_opt(full_year, dt.month(), dt.day());
    }

    if let Ok(dt) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Some(dt);
    }

    None
}

fn parse_time(time_str: &str) -> Option<chrono::NaiveTime> {
    let trimmed = time_str.trim();

    if let Ok(t) = chrono::NaiveTime::parse_from_str(trimmed, "%H:%M:%S") {
        return Some(t);
    }

    if let Ok(t) = chrono::NaiveTime::parse_from_str(trimmed, "%H:%M") {
        return Some(t);
    }

    None
}

fn parse_float(s: &str) -> Option<f64> {
    s.trim().replace(",", "").parse().ok()
}

fn parse_int(s: &str) -> Option<i32> {
    s.trim().replace(",", "").replace(" ", "").parse().ok()
}
