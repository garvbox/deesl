use askama::Template;
use axum::{
    Form,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::patch,
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::Deserialize;

use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::models::{NewVehicle, User, Vehicle};
use crate::schema::{users, vehicles};
use crate::user_handlers::SUPPORTED_CURRENCIES;
use std::collections::HashMap;

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::fmt::Display,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
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

pub async fn update_settings(
    State(pool): State<Pool>,
    user: AuthUser,
    Form(payload): Form<UpdateSettingsForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    crate::user_handlers::validate_currency(&payload.currency)?;

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

    Ok(Html(
        r#"<p style="color: #28a745; font-size: 0.875rem; margin-top: 0.5rem;">Preferences saved successfully!</p>"#,
    ))
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
    pub current_time: String,
}

pub async fn new_fuel_entry(
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

    let template = AddFuelEntryTemplate {
        logged_in: true,
        vehicles: user_vehicles,
        current_time: chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string(),
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Deserialize)]
pub struct CreateFuelEntryForm {
    pub vehicle_id: i32,
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

    // Verify ownership
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
                station_id: None,
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

    if user_vehicles.is_empty() {
        return Ok(Html(
            r#"<p style="color: var(--text-muted);">No vehicles found. Add one to get started!</p>"#.to_string(),
        ).into_response());
    }

    let mut html = String::from(r#"<div style="display: grid; gap: 1rem;">"#);
    for v in user_vehicles {
        let card = VehicleCardTemplate {
            id: v.id,
            make: v.make,
            model: v.model,
            registration: v.registration,
        };
        html.push_str(&card.render().map_err(internal_error)?);
    }
    html.push_str("</div>");

    Ok(Html(html).into_response())
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

pub async fn htmx_recent_entries(
    State(pool): State<Pool>,
    user: AuthUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = user.user_id;

    let entries: Vec<(crate::models::FuelEntry, Vehicle)> = conn
        .interact(move |conn| {
            crate::schema::fuel_entries::table
                .inner_join(vehicles::table)
                .filter(vehicles::owner_id.eq(user_id))
                .order(crate::schema::fuel_entries::filled_at.desc())
                .limit(5)
                .select((crate::models::FuelEntry::as_select(), Vehicle::as_select()))
                .load::<(crate::models::FuelEntry, Vehicle)>(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if entries.is_empty() {
        return Ok(Html(
            r#"<p style="color: var(--text-muted);">No entries yet.</p>"#.to_string(),
        ).into_response());
    }

    let mut html = String::from(
        r#"<table style="width: 100%; border-collapse: collapse; font-size: 0.9rem;">
            <thead>
                <tr style="text-align: left; border-bottom: 1px solid var(--border);">
                    <th style="padding: 0.5rem;">Date</th>
                    <th style="padding: 0.5rem;">Vehicle</th>
                    <th style="padding: 0.5rem;">Litres</th>
                    <th style="padding: 0.5rem;">Cost</th>
                </tr>
            </thead>
            <tbody>"#,
    );

    for (e, v) in entries {
        html.push_str(&format!(
            r#"<tr style="border-bottom: 1px solid var(--border);">
                <td style="padding: 0.5rem;">{}</td>
                <td style="padding: 0.5rem;">{}</td>
                <td style="padding: 0.5rem;">{:.2} L</td>
                <td style="padding: 0.5rem;">€{:.2}</td>
            </tr>"#,
            e.filled_at.format("%Y-%m-%d"),
            v.registration,
            e.litres,
            e.cost
        ));
    }

    html.push_str("</tbody></table>");

    Ok(Html(html).into_response())
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
    pub vehicle_id: i32,
    pub columns: Vec<String>,
    pub sample_data: Vec<String>,
    pub suggested_mappings: HashMap<String, String>,
}

impl ImportMappingTemplate {
    pub fn is_mapped(&self, column: &str, target: &str) -> bool {
        self.suggested_mappings.get(column).map(|s| s == target).unwrap_or(false)
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
                vehicle_id = Some(
                    String::from_utf8_lossy(&data)
                        .trim()
                        .parse()
                        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid vehicle_id".to_string()))?,
                );
            }
            _ => {}
        }
    }

    let vehicle_id =
        vehicle_id.ok_or((StatusCode::BAD_REQUEST, "vehicle_id required".to_string()))?;
    let csv_data = csv_data.ok_or((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))?;

    crate::import_handlers::check_vehicle_write_access(&pool, user.user_id, vehicle_id).await?;

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
        vehicle_id,
        columns: headers,
        sample_data: record,
        suggested_mappings,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

pub async fn htmx_import_execute(
    State(_pool): State<Pool>,
    _user: AuthUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut csv_data: Option<Vec<u8>> = None;
    let mut vehicle_id: Option<i32> = None;
    let mut mappings_raw: HashMap<String, String> = HashMap::new();

    while let Some(field) = multipart.next_field().await.map_err(internal_error)? {
        let name = field.name().unwrap_or("").to_string();
        let data = field.bytes().await.map_err(internal_error)?;

        match name.as_str() {
            "file" => csv_data = Some(data.to_vec()),
            "vehicle_id" => {
                vehicle_id = Some(
                    String::from_utf8_lossy(&data)
                        .trim()
                        .parse()
                        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid vehicle_id".to_string()))?,
                );
            }
            _ if name.starts_with("map_") => {
                mappings_raw.insert(name, String::from_utf8_lossy(&data).to_string());
            }
            _ if name.starts_with("col_") => {
                mappings_raw.insert(name, String::from_utf8_lossy(&data).to_string());
            }
            _ => {}
        }
    }

    let _vehicle_id =
        vehicle_id.ok_or((StatusCode::BAD_REQUEST, "vehicle_id required".to_string()))?;
    let _csv_data = csv_data.ok_or((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))?;

    Ok(Html(format!(
        r#"<div class="card" style="background: #d4edda; border-color: #c3e6cb; color: #155724;">
            <h3>Import Successful</h3>
            <p>Your data has been imported successfully.</p>
            <a href="/dashboard" class="btn">Return to Dashboard</a>
        </div>"#
    )))
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
}
