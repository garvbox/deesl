use askama::Template;
use axum::{
    Form,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::Deserialize;

use crate::auth::{AuthUser, AuthUserRedirect};
use crate::models::{NewVehicle, Vehicle};
use crate::schema::vehicles;

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
                .load(conn)
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
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if user_vehicles.is_empty() {
        return Ok(Html(
            r#"<p style="color: var(--text-muted);">No vehicles found. Add one to get started!</p>"#.to_string(),
        ));
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

    Ok(Html(html))
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
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if entries.is_empty() {
        return Ok(Html(
            r#"<p style="color: var(--text-muted);">No entries yet.</p>"#.to_string(),
        ));
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

    Ok(Html(html))
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
