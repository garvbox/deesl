use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::Deserialize;

use crate::auth::AuthConfig;
use crate::handlers::internal_error;
use crate::models::User;
use crate::schema::users;

pub fn router() -> Router<Pool> {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: i32,
    pub email: String,
}

pub async fn register(
    State(pool): State<Pool>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;

    let password_hash = bcrypt::hash(&payload.password, 10).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to hash password: {}", e),
        )
    })?;

    let email = payload.email.clone();
    let result = conn
        .interact(move |conn| {
            diesel::insert_into(users::table)
                .values((
                    users::email.eq(email.clone()),
                    users::password_hash.eq(password_hash.clone()),
                ))
                .returning(User::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            )
        })?;

    let config = AuthConfig::new();
    let token = config.create_token(result.id, &result.email).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create token: {}", e),
        )
    })?;

    Ok(Json(AuthResponse {
        token,
        user_id: result.id,
        email: result.email,
    }))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(pool): State<Pool>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;

    let email = payload.email.clone();
    let user: User = conn
        .interact(move |conn| {
            users::table
                .filter(users::email.eq(email.clone()))
                .first(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid email or password".to_string(),
            )
        })?;

    let valid = bcrypt::verify(&payload.password, &user.password_hash).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to verify password: {}", e),
        )
    })?;

    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid email or password".to_string(),
        ));
    }

    let config = AuthConfig::new();
    let token = config.create_token(user.id, &user.email).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create token: {}", e),
        )
    })?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        email: user.email,
    }))
}
