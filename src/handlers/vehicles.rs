use askama::Template;
use axum::{
    Form, Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get},
};
use diesel::prelude::*;
use serde::Deserialize;

use super::{HxRedirect, internal_error};
use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::db::DbConn;
use crate::models::{NewVehicle, Vehicle};
use crate::schema::vehicles;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(vehicles_page).post(create_vehicle))
        .route("/new", get(new_vehicle))
        .route("/htmx/list", get(htmx_vehicles))
        .route("/htmx/{id}", delete(htmx_delete_vehicle))
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
    DbConn(conn): DbConn,
    user: AuthUser,
    Form(payload): Form<CreateVehicleForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
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

    Ok(HxRedirect("/dashboard"))
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
    DbConn(conn): DbConn,
    user: AuthUser,
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

    let template = VehicleListTemplate {
        vehicles: user_vehicles,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

pub async fn htmx_delete_vehicle(
    DbConn(conn): DbConn,
    user: AuthUser,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
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
