use askama::Template;
use axum::{
    Form, Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use diesel::prelude::*;
use serde::Deserialize;

use super::{SUPPORTED_CURRENCIES, internal_error, validate_currency};
use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::db::DbConn;
use crate::models::User;
use crate::schema::users;

// ... (rest of structs)

pub fn router() -> Router<AppState> {
    Router::new().route("/settings", get(settings_page).patch(update_settings))
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub logged_in: bool,
    pub user_email: String,
    pub current_currency: String,
    pub currencies: Vec<String>,
    pub current_distance_unit: String,
    pub current_volume_unit: String,
}

impl SettingsTemplate {
    pub fn is_current_currency(&self, currency: &str) -> bool {
        self.current_currency == currency
    }

    pub fn is_current_distance_unit(&self, unit: &str) -> bool {
        self.current_distance_unit == unit
    }

    pub fn is_current_volume_unit(&self, unit: &str) -> bool {
        self.current_volume_unit == unit
    }
}

pub async fn settings_page(
    DbConn(conn): DbConn,
    AuthUserRedirect(user): AuthUserRedirect,
) -> Result<impl IntoResponse, (StatusCode, String)> {
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
        current_distance_unit: db_user.distance_unit,
        current_volume_unit: db_user.volume_unit,
    };
    Ok(Html(template.render().map_err(internal_error)?))
}

#[derive(Deserialize)]
pub struct UpdateSettingsForm {
    pub currency: String,
    pub distance_unit: String,
    pub volume_unit: String,
}

#[derive(Template)]
#[template(path = "fragments/settings_success.html")]
pub struct SettingsSuccessTemplate {}

pub async fn update_settings(
    DbConn(conn): DbConn,
    user: AuthUser,
    Form(payload): Form<UpdateSettingsForm>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    validate_currency(&payload.currency)?;

    // Basic unit validation
    if !["km", "mi"].contains(&payload.distance_unit.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid distance unit".to_string()));
    }
    if !["L", "gal"].contains(&payload.volume_unit.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid volume unit".to_string()));
    }

    let user_id = user.user_id;

    conn.interact(move |conn| {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(crate::models::UpdateUser {
                currency: Some(payload.currency),
                distance_unit: Some(payload.distance_unit),
                volume_unit: Some(payload.volume_unit),
            })
            .execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let template = SettingsSuccessTemplate {};
    Ok(Html(template.render().map_err(internal_error)?))
}
