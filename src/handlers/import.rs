use askama::Template;
use axum::{
    Form, Router,
    extract::Multipart,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use chrono::Datelike;
use diesel::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

use super::{check_vehicle_write_access, internal_error};
use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::db::DbConn;
use crate::models::{FuelStation, NewFuelStation, Vehicle};
use crate::schema::{fuel_stations, vehicles};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(import_page))
        .route("/htmx/preview", post(htmx_import_preview))
        .route("/htmx/execute", post(htmx_import_execute))
}

#[derive(Serialize, Clone)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub stations_created: usize,
    pub total_errors: usize,
    pub errors: Vec<String>,
}

#[derive(Clone)]
struct ParsedRow {
    #[allow(dead_code)]
    row_num: usize,
    filled_at: chrono::NaiveDateTime,
    station_name: Option<String>,
    litres: f64,
    cost: f64,
    mileage_km: i32,
}

#[derive(Clone)]
struct StationOp {
    normalized_name: String,
    original_name: String,
}

pub async fn perform_import(
    conn: &deadpool_diesel::postgres::Object,
    user_id: i32,
    vehicle_id: i32,
    csv_data: Vec<u8>,
    mappings: HashMap<String, String>,
) -> Result<ImportResult, (StatusCode, String)> {
    check_vehicle_write_access(conn, user_id, vehicle_id).await?;

    let mut reader = csv::Reader::from_reader(&csv_data[..]);
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("CSV parse error: {}", e)))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let date_col = mappings.get("filled_at_date").cloned();
    let time_col = mappings.get("filled_at_time").cloned();
    let station_col = mappings.get("station").cloned();
    let litres_col = mappings.get("litres").cloned();
    let cost_col = mappings.get("cost").cloned();
    let km_col = mappings.get("mileage_km").cloned();

    let mut parsed_rows: Vec<ParsedRow> = Vec::new();
    let mut parse_errors: Vec<String> = Vec::new();

    for (row_num, result) in reader.records().enumerate() {
        let row_num = row_num + 2;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                parse_errors.push(format!("Row {}: CSV parse error - {}", row_num, e));
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
            parse_errors.push(format!(
                "Row {}: Missing required fields (date, litres, cost, or mileage)",
                row_num
            ));
            continue;
        }

        let filled_at = match parse_datetime(date_str.as_deref().unwrap(), time_str.as_deref()) {
            Some(dt) => dt,
            None => {
                parse_errors.push(format!(
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
                parse_errors.push(format!(
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
                parse_errors.push(format!(
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
                parse_errors.push(format!(
                    "Row {}: Invalid mileage value: {}",
                    row_num,
                    km_str.as_deref().unwrap_or("")
                ));
                continue;
            }
        };

        parsed_rows.push(ParsedRow {
            row_num,
            filled_at,
            station_name: station_name.filter(|s| !s.trim().is_empty()),
            litres,
            cost,
            mileage_km,
        });
    }

    if !parse_errors.is_empty() {
        return Ok(ImportResult {
            imported: 0,
            skipped: 0,
            stations_created: 0,
            total_errors: parse_errors.len(),
            errors: parse_errors.into_iter().take(10).collect(),
        });
    }

    let existing_stations: HashMap<String, i32> = conn
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

    let mut stations_to_create: Vec<StationOp> = Vec::new();
    let stations_map = existing_stations;
    let mut seen_stations: std::collections::HashSet<String> = std::collections::HashSet::new();

    for row in &parsed_rows {
        if let Some(name) = &row.station_name {
            let normalized = normalize_station_name(name);
            if !stations_map.contains_key(&normalized) && !seen_stations.contains(&normalized) {
                seen_stations.insert(normalized.clone());
                stations_to_create.push(StationOp {
                    normalized_name: normalized,
                    original_name: name.trim().to_string(),
                });
            }
        }
    }

    let result: (usize, usize, usize, Vec<String>) = conn
        .interact(move |conn| {
            conn.transaction::<_, diesel::result::Error, _>(move |conn| {
                use diesel::result::Error as DieselError;

                let mut stations_created: usize = 0;
                let mut new_stations_cache: HashMap<String, i32> = HashMap::new();

                for station_op in stations_to_create {
                    let new_station_name = station_op.original_name.clone();
                    match diesel::insert_into(fuel_stations::table)
                        .values(NewFuelStation {
                            name: new_station_name,
                            user_id: Some(user_id),
                        })
                        .returning(fuel_stations::id)
                        .get_result::<i32>(conn)
                    {
                        Ok(id) => {
                            new_stations_cache.insert(station_op.normalized_name.clone(), id);
                            stations_created += 1;
                        }
                        Err(DieselError::DatabaseError(
                            diesel::result::DatabaseErrorKind::UniqueViolation,
                            _,
                        )) => {
                            if let Ok(id) = fuel_stations::table
                                .filter(fuel_stations::name.eq(station_op.original_name.trim()))
                                .filter(
                                    fuel_stations::user_id
                                        .eq(user_id)
                                        .or(fuel_stations::user_id.is_null()),
                                )
                                .select(fuel_stations::id)
                                .first::<i32>(conn)
                            {
                                new_stations_cache.insert(station_op.normalized_name.clone(), id);
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }

                let mut all_stations = stations_map;
                all_stations.extend(new_stations_cache);

                let mut imported: usize = 0;
                let mut skipped: usize = 0;
                let mut row_errors: Vec<String> = Vec::new();

                for row in parsed_rows {
                    let station_id = row
                        .station_name
                        .as_ref()
                        .and_then(|name| all_stations.get(&normalize_station_name(name)).copied());

                    let exists: bool = diesel::dsl::select(diesel::dsl::exists(
                        crate::schema::fuel_entries::table
                            .filter(crate::schema::fuel_entries::vehicle_id.eq(vehicle_id))
                            .filter(crate::schema::fuel_entries::mileage_km.eq(row.mileage_km))
                            .filter(crate::schema::fuel_entries::filled_at.eq(row.filled_at)),
                    ))
                    .get_result(conn)
                    .unwrap_or(false);

                    if exists {
                        skipped += 1;
                        continue;
                    }

                    match diesel::insert_into(crate::schema::fuel_entries::table)
                        .values((
                            crate::schema::fuel_entries::vehicle_id.eq(vehicle_id),
                            crate::schema::fuel_entries::station_id.eq(station_id),
                            crate::schema::fuel_entries::mileage_km.eq(row.mileage_km),
                            crate::schema::fuel_entries::litres.eq(row.litres),
                            crate::schema::fuel_entries::cost.eq(row.cost),
                            crate::schema::fuel_entries::filled_at.eq(row.filled_at),
                        ))
                        .execute(conn)
                    {
                        Ok(_) => imported += 1,
                        Err(e) => {
                            row_errors.push(format!("Row {}: Database error - {}", row.row_num, e));
                        }
                    }
                }

                Ok((imported, skipped, stations_created, row_errors))
            })
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Transaction failed: {}", e),
            )
        })?;

    let (imported, skipped, stations_created, mut row_errors) = result;
    let mut all_errors = parse_errors;
    all_errors.append(&mut row_errors);
    let total_errors = all_errors.len();

    Ok(ImportResult {
        imported,
        skipped,
        stations_created,
        total_errors,
        errors: all_errors.into_iter().take(10).collect(),
    })
}

#[derive(Template)]
#[template(path = "import.html")]
pub struct ImportTemplate {
    pub logged_in: bool,
    pub vehicles: Vec<Vehicle>,
}

pub async fn import_page(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    let user_vehicles: Vec<Vehicle> = conn
        .interact(move |conn| {
            vehicles::table
                .filter(vehicles::owner_id.eq(user_id))
                .order(vehicles::registration)
                .load::<Vehicle>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = ImportTemplate {
        logged_in: true,
        vehicles: user_vehicles,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Template)]
#[template(path = "fragments/import_mapping.html")]
pub struct ImportMappingTemplate {
    pub import_id: String,
    pub vehicle_id: i32,
    pub columns: Vec<String>,
    pub sample_data: Vec<String>,
    pub suggested_mappings: HashMap<String, String>,
}

impl ImportMappingTemplate {
    pub fn is_mapped(&self, column: &str, target: &str) -> bool {
        self.suggested_mappings
            .get(column)
            .map(|s| s == target)
            .unwrap_or(false)
    }
}

pub async fn htmx_import_preview(
    DbConn(conn): DbConn,
    user: AuthUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut csv_data: Option<Vec<u8>> = None;
    let mut vehicle_id: Option<i32> = None;

    while let Some(field) = multipart.next_field().await.map_err(internal_error)? {
        let name = field.name().unwrap_or("").to_string();
        let data = field.bytes().await.map_err(internal_error)?;

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

    check_vehicle_write_access(&conn, user.user_id, vehicle_id).await?;

    // Clean up any old imports for this user first
    let user_id = user.user_id;
    let _ = conn
        .interact(move |conn| {
            diesel::delete(
                crate::schema::temp_imports::table
                    .filter(crate::schema::temp_imports::user_id.eq(user_id))
                    .filter(
                        crate::schema::temp_imports::created_at
                            .lt(chrono::Utc::now().naive_utc() - chrono::Duration::hours(1)),
                    ),
            )
            .execute(conn)
        })
        .await
        .map_err(internal_error)?;

    // Store CSV in temp_imports table
    let csv_data_clone = csv_data.clone();
    let temp_import: crate::models::TempImport = conn
        .interact(move |conn| {
            diesel::insert_into(crate::schema::temp_imports::table)
                .values(crate::models::NewTempImport {
                    user_id,
                    vehicle_id,
                    csv_data: csv_data_clone,
                })
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut reader = csv::Reader::from_reader(&csv_data[..]);
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("CSV parse error: {}", e)))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let record = reader
        .records()
        .next()
        .transpose()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("CSV parse error: {}", e)))?
        .map(|r| r.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .unwrap_or_else(|| vec!["".to_string(); headers.len()]);

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

    let template = ImportMappingTemplate {
        import_id: temp_import.id.to_string(),
        vehicle_id,
        columns: headers,
        sample_data: record,
        suggested_mappings,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Template)]
#[template(path = "fragments/import_result.html")]
pub struct ImportResultTemplate {
    pub success: bool,
    pub imported: usize,
    pub skipped: usize,
    pub stations_created: usize,
    pub total_errors: usize,
    pub errors: Vec<String>,
}

pub async fn htmx_import_execute(
    DbConn(conn): DbConn,
    user: AuthUser,
    Form(payload): Form<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = user.user_id;

    // Extract import_id and vehicle_id from form
    let import_id = payload
        .get("import_id")
        .ok_or((StatusCode::BAD_REQUEST, "import_id required".to_string()))?;
    let vehicle_id = payload
        .get("vehicle_id")
        .ok_or((StatusCode::BAD_REQUEST, "vehicle_id required".to_string()))?
        .parse::<i32>()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid vehicle_id".to_string()))?;

    // Parse import_id as UUID
    let import_uuid = uuid::Uuid::parse_str(import_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid import_id format".to_string(),
        )
    })?;

    // Retrieve CSV data from temp_imports table and verify ownership
    let temp_import: crate::models::TempImport = conn
        .interact(move |conn| {
            crate::schema::temp_imports::table
                .filter(crate::schema::temp_imports::id.eq(import_uuid))
                .filter(crate::schema::temp_imports::user_id.eq(user_id))
                .first::<crate::models::TempImport>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                "Import not found or expired".to_string(),
            )
        })?;

    // Build mappings from form data
    let mut actual_mappings: HashMap<String, String> = HashMap::new();
    let mut used_targets: HashMap<String, String> = HashMap::new(); // target -> column
    let mut i = 0;
    while let Some(col_name) = payload.get(&format!("col_{}", i)) {
        if let Some(target_field) = payload.get(&format!("map_{}", i)).filter(|s| !s.is_empty()) {
            // Check if this target is already mapped to a different column
            if let Some(existing_col) = used_targets.get(target_field)
                && existing_col != col_name
            {
                let template = ImportResultTemplate {
                    success: false,
                    imported: 0,
                    skipped: 0,
                    stations_created: 0,
                    total_errors: 1,
                    errors: vec![format!(
                        "Field '{}' is mapped multiple times: '{}' and '{}'",
                        target_field, existing_col, col_name
                    )],
                };
                return Ok(Html(template.render().map_err(internal_error)?));
            }
            used_targets.insert(target_field.clone(), col_name.clone());
            actual_mappings.insert(target_field.clone(), col_name.clone());
        }
        i += 1;
    }

    // Perform the import
    let csv_data = temp_import.csv_data;
    let result = perform_import(&conn, user.user_id, vehicle_id, csv_data, actual_mappings).await?;

    // Clean up the temp import after successful/failed execution
    let import_uuid_clone = import_uuid;
    let _ = conn
        .interact(move |conn| {
            diesel::delete(
                crate::schema::temp_imports::table
                    .filter(crate::schema::temp_imports::id.eq(import_uuid_clone)),
            )
            .execute(conn)
        })
        .await
        .map_err(internal_error)?;

    let template = if result.total_errors > 0 && result.imported == 0 {
        ImportResultTemplate {
            success: false,
            imported: 0,
            skipped: 0,
            stations_created: 0,
            total_errors: result.total_errors,
            errors: result.errors,
        }
    } else {
        ImportResultTemplate {
            success: true,
            imported: result.imported,
            skipped: result.skipped,
            stations_created: result.stations_created,
            total_errors: result.total_errors,
            errors: result.errors,
        }
    };

    Ok(Html(template.render().map_err(internal_error)?))
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
