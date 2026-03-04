use askama::Template;
use axum::{
    Form,
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use chrono::Datelike;
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::auth::{AuthUser, AuthUserRedirect};
use crate::models::{FuelEntry, FuelStation, NewFuelStation, NewVehicle, User, Vehicle};
use crate::schema::{fuel_stations, users, vehicles};

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::fmt::Display,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub const SUPPORTED_CURRENCIES: &[&str] = &["EUR", "GBP", "USD", "CAD", "AUD"];

pub fn validate_currency(currency: &str) -> Result<(), (StatusCode, String)> {
    if SUPPORTED_CURRENCIES.contains(&currency) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Unsupported currency '{}'. Must be one of: {}",
                currency,
                SUPPORTED_CURRENCIES.join(", ")
            ),
        ))
    }
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

pub async fn check_vehicle_write_access(
    pool: &Pool,
    user_id: i32,
    vehicle_id: i32,
) -> Result<(), (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let has_write_access: (bool, bool) = conn
        .interact(move |conn| {
            let vehicle: Vehicle = vehicles::table
                .filter(vehicles::id.eq(vehicle_id))
                .first(conn)
                .optional()?
                .ok_or(diesel::result::Error::NotFound)?;

            if vehicle.owner_id == user_id {
                return Ok::<(bool, bool), diesel::result::Error>((true, true));
            }

            let share = crate::schema::vehicle_shares::table
                .filter(crate::schema::vehicle_shares::vehicle_id.eq(vehicle_id))
                .filter(crate::schema::vehicle_shares::shared_with_user_id.eq(user_id))
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

    Ok(())
}

pub async fn perform_import(
    pool: &Pool,
    user_id: i32,
    vehicle_id: i32,
    csv_data: Vec<u8>,
    mappings: HashMap<String, String>,
) -> Result<ImportResult, (StatusCode, String)> {
    check_vehicle_write_access(pool, user_id, vehicle_id).await?;

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

    let conn = pool.get().await.map_err(internal_error)?;
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

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub logged_in: bool,
}

pub async fn login() -> impl IntoResponse {
    let template = LoginTemplate { logged_in: false };
    Html(template.render().unwrap())
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub logged_in: bool,
    pub user_email: String,
}

pub async fn dashboard(AuthUserRedirect(user): AuthUserRedirect) -> impl IntoResponse {
    let template = DashboardTemplate {
        logged_in: true,
        user_email: user.email,
    };
    Html(template.render().unwrap())
}

#[derive(Template)]
#[template(path = "stats.html")]
pub struct StatsTemplate {
    pub logged_in: bool,
    pub has_data: bool,
    pub total_entries: usize,
    pub total_litres: f64,
    pub total_cost: f64,
    pub avg_efficiency: f64,
    pub avg_cost_per_km: f64,
    pub labels: Vec<String>,
    pub efficiency_data: Vec<f64>,
    pub cost_per_km_data: Vec<f64>,
    pub cost_per_litre_data: Vec<f64>,
    pub vehicle_stats: Vec<VehicleStat>,
}

#[derive(Serialize)]
pub struct VehicleStat {
    pub registration: String,
    pub make: String,
    pub model: String,
    pub total_entries: i64,
    pub total_km: f64,
    pub total_litres: f64,
    pub avg_efficiency: f64,
    pub total_cost: f64,
}

pub async fn stats_page(
    State(pool): State<Pool>,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    // Get all fuel entries for the user's vehicles with vehicle and station info
    let entries: Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)> = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .left_join(crate::schema::fuel_stations::table)
                .filter(vehicles::owner_id.eq(user_id))
                .order(crate::schema::fuel_entries::filled_at.asc())
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

    if entries.is_empty() {
        let template = StatsTemplate {
            logged_in: true,
            has_data: false,
            total_entries: 0,
            total_litres: 0.0,
            total_cost: 0.0,
            avg_efficiency: 0.0,
            avg_cost_per_km: 0.0,
            labels: vec![],
            efficiency_data: vec![],
            cost_per_km_data: vec![],
            cost_per_litre_data: vec![],
            vehicle_stats: vec![],
        };
        return Ok(Html(template.render().map_err(internal_error)?));
    }

    // Calculate overall stats
    let total_entries = entries.len();
    let total_litres: f64 = entries.iter().map(|(e, _, _)| e.litres).sum();
    let total_cost: f64 = entries.iter().map(|(e, _, _)| e.cost).sum();

    // Group entries by vehicle for efficiency calculations
    let mut vehicle_data: HashMap<
        i32,
        Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>,
    > = HashMap::new();
    for (entry, vehicle, station) in entries {
        vehicle_data
            .entry(vehicle.id)
            .or_default()
            .push((entry, vehicle.clone(), station));
    }

    // Calculate vehicle stats and chart data
    let mut vehicle_stats = Vec::new();
    let mut chart_entries: Vec<(String, f64, f64, f64)> = Vec::new(); // (date, efficiency, cost_per_km, cost_per_litre)

    for (_, vehicle_entries) in vehicle_data.iter() {
        let vehicle = &vehicle_entries[0].1;
        let mut total_km = 0.0;
        let mut vehicle_litres = 0.0;
        let mut vehicle_cost = 0.0;
        let mut prev_mileage: Option<i32> = None;
        let mut efficiency_sum = 0.0;
        let mut efficiency_count = 0;

        for (entry, _, _) in vehicle_entries.iter() {
            vehicle_litres += entry.litres;
            vehicle_cost += entry.cost;

            if let Some(prev) = prev_mileage {
                let km = (entry.mileage_km - prev) as f64;
                if km > 0.0 && entry.litres > 0.0 {
                    let efficiency = km / entry.litres; // km per litre
                    let cost_per_km = entry.cost / km;
                    let cost_per_litre = entry.cost / entry.litres;
                    efficiency_sum += efficiency;
                    efficiency_count += 1;
                    total_km += km;

                    let date_label = entry.filled_at.format("%Y-%m-%d").to_string();
                    chart_entries.push((date_label, efficiency, cost_per_km, cost_per_litre));
                }
            }
            prev_mileage = Some(entry.mileage_km);
        }

        let avg_efficiency = if efficiency_count > 0 {
            efficiency_sum / efficiency_count as f64
        } else {
            0.0
        };

        vehicle_stats.push(VehicleStat {
            registration: vehicle.registration.clone(),
            make: vehicle.make.clone(),
            model: vehicle.model.clone(),
            total_entries: vehicle_entries.len() as i64,
            total_km,
            total_litres: vehicle_litres,
            avg_efficiency,
            total_cost: vehicle_cost,
        });
    }

    // Sort chart entries by date and take last 20
    chart_entries.sort_by(|a, b| a.0.cmp(&b.0));
    let chart_entries: Vec<_> = chart_entries.into_iter().rev().take(20).collect();

    let labels: Vec<String> = chart_entries.iter().map(|(d, _, _, _)| d.clone()).collect();
    let efficiency_data: Vec<f64> = chart_entries.iter().map(|(_, e, _, _)| *e).collect();
    let cost_per_km_data: Vec<f64> = chart_entries.iter().map(|(_, _, c, _)| *c).collect();
    let cost_per_litre_data: Vec<f64> = chart_entries.iter().map(|(_, _, _, l)| *l).collect();

    // Calculate averages
    let avg_efficiency = if !efficiency_data.is_empty() {
        efficiency_data.iter().sum::<f64>() / efficiency_data.len() as f64
    } else {
        0.0
    };

    let avg_cost_per_km = if !cost_per_km_data.is_empty() {
        cost_per_km_data.iter().sum::<f64>() / cost_per_km_data.len() as f64
    } else {
        0.0
    };

    let template = StatsTemplate {
        logged_in: true,
        has_data: true,
        total_entries,
        total_litres,
        total_cost,
        avg_efficiency,
        avg_cost_per_km,
        labels,
        efficiency_data,
        cost_per_km_data,
        cost_per_litre_data,
        vehicle_stats,
    };

    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub logged_in: bool,
    pub user_email: String,
    pub current_currency: String,
    pub currencies: Vec<String>,
}

impl SettingsTemplate {
    pub fn is_current_currency(&self, currency: &str) -> bool {
        self.current_currency == currency
    }
}

pub async fn settings_page(
    State(pool): State<Pool>,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    let db_user: User = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(user_id))
                .first::<User>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = SettingsTemplate {
        logged_in: true,
        user_email: db_user.email,
        current_currency: db_user.currency,
        currencies: SUPPORTED_CURRENCIES.iter().map(|s| s.to_string()).collect(),
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Deserialize)]
pub struct UpdateSettingsForm {
    pub currency: String,
}

#[derive(Template)]
#[template(path = "fragments/settings_success.html")]
pub struct SettingsSuccessTemplate {}

pub async fn update_settings(
    State(pool): State<Pool>,
    user: AuthUser,
    Form(payload): Form<UpdateSettingsForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    validate_currency(&payload.currency)?;

    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;
    let currency = payload.currency.clone();

    conn.interact(move |conn| {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::currency.eq(currency))
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = SettingsSuccessTemplate {};
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Template)]
#[template(path = "add_vehicle.html")]
pub struct AddVehicleTemplate {
    pub logged_in: bool,
}

pub async fn new_vehicle(AuthUserRedirect(_user): AuthUserRedirect) -> impl IntoResponse {
    let template = AddVehicleTemplate { logged_in: true };
    Html(template.render().unwrap())
}

#[derive(Template)]
#[template(path = "vehicles.html")]
pub struct VehiclesTemplate {
    pub logged_in: bool,
}

pub async fn vehicles_page(AuthUserRedirect(_user): AuthUserRedirect) -> impl IntoResponse {
    let template = VehiclesTemplate { logged_in: true };
    Html(template.render().unwrap())
}

#[derive(Deserialize)]
pub struct CreateVehicleForm {
    pub make: String,
    pub model: String,
    pub registration: String,
}

pub async fn create_vehicle(
    State(pool): State<Pool>,
    user: AuthUser,
    Form(payload): Form<CreateVehicleForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let owner_id = user.user_id;

    conn.interact(move |conn| {
        diesel::insert_into(vehicles::table)
            .values(NewVehicle {
                make: payload.make,
                model: payload.model,
                registration: payload.registration,
                owner_id,
            })
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", "/dashboard".parse().unwrap());
    Ok((headers, Redirect::to("/dashboard")))
}

#[derive(Template)]
#[template(path = "add_fuel_entry.html")]
pub struct AddFuelEntryTemplate {
    pub logged_in: bool,
    pub vehicles: Vec<Vehicle>,
    pub stations: Vec<FuelStation>,
    pub current_time: String,
}

pub async fn new_fuel_entry(
    State(pool): State<Pool>,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    let (user_vehicles, user_stations): (Vec<Vehicle>, Vec<FuelStation>) = conn
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
            Ok::<(Vec<Vehicle>, Vec<FuelStation>), diesel::result::Error>((vehicles, stations))
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = AddFuelEntryTemplate {
        logged_in: true,
        vehicles: user_vehicles,
        stations: user_stations,
        current_time: chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string(),
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
    State(pool): State<Pool>,
    user: AuthUser,
    Form(payload): Form<CreateFuelEntryForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;
    let vehicle_id = payload.vehicle_id;

    let is_owner: bool = conn
        .interact(move |conn| {
            vehicles::table
                .filter(vehicles::id.eq(vehicle_id))
                .filter(vehicles::owner_id.eq(user_id))
                .first::<Vehicle>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_some();

    if !is_owner {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

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

    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", "/dashboard".parse().unwrap());
    Ok((headers, Redirect::to("/dashboard")))
}

#[derive(Template)]
#[template(path = "fragments/vehicle_card.html")]
pub struct VehicleCardTemplate {
    pub id: i32,
    pub make: String,
    pub model: String,
    pub registration: String,
}

#[derive(Template)]
#[template(path = "fragments/vehicle_list.html")]
pub struct VehicleListTemplate {
    pub vehicles: Vec<Vehicle>,
}

pub async fn htmx_vehicles(
    State(pool): State<Pool>,
    user: AuthUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
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

    let template = VehicleListTemplate {
        vehicles: user_vehicles,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

pub async fn htmx_delete_vehicle(
    State(pool): State<Pool>,
    user: AuthUser,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    conn.interact(move |conn| {
        diesel::delete(
            vehicles::table
                .filter(vehicles::id.eq(id))
                .filter(vehicles::owner_id.eq(user_id)),
        )
        .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Html(""))
}

pub async fn htmx_delete_fuel_entry(
    State(pool): State<Pool>,
    user: AuthUser,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    // First check if entry exists and user owns the vehicle
    let is_owner: bool = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(crate::schema::fuel_entries::id.eq(id))
                .filter(vehicles::owner_id.eq(user_id))
                .select(crate::schema::fuel_entries::id)
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
}

impl RecentEntriesTemplate {
    pub fn format_float(&self, val: &f64) -> String {
        format!("{:.2}", val)
    }
}

pub async fn htmx_recent_entries(
    State(pool): State<Pool>,
    user: AuthUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    let entries: Vec<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)> = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
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
                .load::<(crate::models::FuelEntry, Vehicle, Option<FuelStation>)>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = RecentEntriesTemplate { entries };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Template)]
#[template(path = "fragments/station_search_results.html")]
pub struct StationSearchResultsTemplate {
    pub stations: Vec<FuelStation>,
    pub query: String,
}

pub async fn htmx_station_search(
    State(pool): State<Pool>,
    user: AuthUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let query = params.get("q").cloned().unwrap_or_default();
    let query_lower = query.to_lowercase();

    if query.len() < 2 {
        return Ok(Html(String::new()));
    }

    let conn = pool.get().await.map_err(internal_error)?;
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

/// Simple fuzzy matching - checks if all characters in query appear in name in order
fn fuzzy_match(name: &str, query: &str) -> bool {
    let mut name_chars = name.chars();
    for query_char in query.chars() {
        loop {
            match name_chars.next() {
                Some(name_char) if name_char == query_char => break,
                None => return false,
                _ => continue,
            }
        }
    }
    true
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
}

pub async fn edit_fuel_entry(
    State(pool): State<Pool>,
    AuthUserRedirect(user): AuthUserRedirect,
    axum::extract::Path(entry_id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    let (result, stations): (Option<(FuelEntry, Vehicle)>, Vec<FuelStation>) = conn
        .interact(move |conn| {
            let entry_result = crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(crate::schema::fuel_entries::id.eq(entry_id))
                .filter(vehicles::owner_id.eq(user_id))
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
            Ok::<(Option<(FuelEntry, Vehicle)>, Vec<FuelStation>), diesel::result::Error>((
                entry_result,
                stations,
            ))
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (entry, vehicle) = result.ok_or((StatusCode::NOT_FOUND, "Entry not found".to_string()))?;

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
    State(pool): State<Pool>,
    user: AuthUser,
    axum::extract::Path(entry_id): axum::extract::Path<i32>,
    Form(payload): Form<UpdateFuelEntryForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    let is_owner: bool = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(crate::schema::fuel_entries::id.eq(entry_id))
                .filter(vehicles::owner_id.eq(user_id))
                .select(crate::schema::fuel_entries::id)
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

    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", "/dashboard".parse().unwrap());
    Ok((headers, Redirect::to("/dashboard")))
}

#[derive(Template)]
#[template(path = "import.html")]
pub struct ImportTemplate {
    pub logged_in: bool,
    pub vehicles: Vec<Vehicle>,
}

pub async fn import_page(
    State(pool): State<Pool>,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
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
    State(pool): State<Pool>,
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

    check_vehicle_write_access(&pool, user.user_id, vehicle_id).await?;

    // Clean up any old imports for this user first
    let user_id = user.user_id;
    let conn = pool.get().await.map_err(internal_error)?;
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
    let conn = pool.get().await.map_err(internal_error)?;
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
    State(pool): State<Pool>,
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
    let conn = pool.get().await.map_err(internal_error)?;
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
    let result = perform_import(&pool, user.user_id, vehicle_id, csv_data, actual_mappings).await?;

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

#[derive(Template)]
#[template(path = "stations.html")]
pub struct StationsTemplate {
    pub logged_in: bool,
    pub stations: Vec<FuelStation>,
}

pub async fn stations_page(
    State(pool): State<Pool>,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
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
    State(pool): State<Pool>,
    user: AuthUser,
    Form(payload): Form<CreateStationForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
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

    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", "/stations".parse().unwrap());
    Ok((headers, Redirect::to("/stations")))
}

#[derive(Deserialize)]
pub struct UpdateStationForm {
    pub name: String,
}

pub async fn update_station(
    State(pool): State<Pool>,
    user: AuthUser,
    axum::extract::Path(station_id): axum::extract::Path<i32>,
    Form(payload): Form<UpdateStationForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
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

    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", "/stations".parse().unwrap());
    Ok((headers, Redirect::to("/stations")))
}

pub async fn delete_station(
    State(pool): State<Pool>,
    user: AuthUser,
    axum::extract::Path(station_id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
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
    fn test_validate_currency_accepts_all_supported_currencies() {
        for currency in SUPPORTED_CURRENCIES {
            assert!(
                validate_currency(currency).is_ok(),
                "{currency} should be accepted"
            );
        }
    }

    #[test]
    fn test_validate_currency_rejects_unknown_currency() {
        let result = validate_currency("XYZ");
        assert!(result.is_err());
        let (status, msg) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("XYZ"));
        assert!(msg.contains("Must be one of"));
    }
}
