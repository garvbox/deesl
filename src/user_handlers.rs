use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::handlers::internal_error;
use crate::models::User;
use crate::schema::users;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/users/me", get(get_me).patch(update_me))
}

#[derive(Serialize)]
pub struct UserProfileResponse {
    pub id: i32,
    pub email: String,
    pub currency: String,
}

impl From<User> for UserProfileResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            currency: u.currency,
        }
    }
}

#[derive(Deserialize)]
pub struct UserQueryParams {
    pub user_id: i32,
}

pub async fn get_me(
    State(pool): State<Pool>,
    axum::extract::Query(params): axum::extract::Query<UserQueryParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = params.user_id;

    let user: User = conn
        .interact(move |conn| users::table.filter(users::id.eq(user_id)).first(conn))
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    Ok(Json(UserProfileResponse::from(user)))
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub currency: String,
}

const SUPPORTED_CURRENCIES: &[&str] = &["EUR", "GBP", "USD", "CAD", "AUD"];

pub async fn update_me(
    State(pool): State<Pool>,
    axum::extract::Query(params): axum::extract::Query<UserQueryParams>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if !SUPPORTED_CURRENCIES.contains(&payload.currency.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Unsupported currency '{}'. Must be one of: {}",
                payload.currency,
                SUPPORTED_CURRENCIES.join(", ")
            ),
        ));
    }

    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = params.user_id;
    let currency = payload.currency.clone();

    let user: User = conn
        .interact(move |conn| {
            diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::currency.eq(currency))
                .returning(User::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    Ok(Json(UserProfileResponse::from(user)))
}
