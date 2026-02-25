use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;

use crate::AppState;
use crate::auth::AuthConfig;
use crate::handlers::internal_error;
use crate::models::{NewVehicleShare, User, VehicleShare};
use crate::oauth_handlers::extract_cookie;
use crate::schema::{users, vehicle_shares, vehicles};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/vehicle-shares",
            get(list_shared_vehicles).post(create_share),
        )
        .route("/api/vehicle-shares/{id}", delete(delete_share))
        .route("/api/vehicle-shares/owned", get(list_owned_vehicle_shares))
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i32,
    #[allow(dead_code)]
    pub email: String,
}

fn extract_auth_user(headers: &HeaderMap) -> Result<AuthUser, (StatusCode, String)> {
    let token = extract_cookie(headers, "auth_token")
        .ok_or((StatusCode::UNAUTHORIZED, "Missing auth token".to_string()))?;

    let auth_config = AuthConfig::new();
    let claims = auth_config
        .validate_token(&token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token".to_string()))?;

    Ok(AuthUser {
        user_id: claims.user_id,
        email: claims.sub,
    })
}

#[derive(serde::Serialize)]
pub struct VehicleShareResponse {
    pub id: i32,
    pub vehicle_id: i32,
    pub vehicle_make: String,
    pub vehicle_model: String,
    pub vehicle_registration: String,
    pub owner_email: String,
    pub permission_level: String,
}

pub async fn list_shared_vehicles(
    State(pool): State<Pool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    let shares: Vec<(VehicleShare, String, String, String, String)> = conn
        .interact(move |conn| {
            vehicle_shares::table
                .inner_join(vehicles::table)
                .inner_join(users::table.on(vehicles::owner_id.eq(users::id)))
                .filter(vehicle_shares::shared_with_user_id.eq(user_id))
                .select((
                    VehicleShare::as_select(),
                    vehicles::make,
                    vehicles::model,
                    vehicles::registration,
                    users::email,
                ))
                .order(vehicle_shares::created_at.desc())
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let response: Vec<VehicleShareResponse> = shares
        .into_iter()
        .map(
            |(share, make, model, registration, owner_email)| VehicleShareResponse {
                id: share.id,
                vehicle_id: share.vehicle_id,
                vehicle_make: make,
                vehicle_model: model,
                vehicle_registration: registration,
                owner_email,
                permission_level: share.permission_level,
            },
        )
        .collect();

    Ok(Json(response))
}

#[derive(serde::Serialize)]
pub struct OwnedVehicleShareResponse {
    pub id: i32,
    pub vehicle_id: i32,
    pub vehicle_make: String,
    pub vehicle_model: String,
    pub vehicle_registration: String,
    pub shared_with_email: String,
    pub permission_level: String,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn list_owned_vehicle_shares(
    State(pool): State<Pool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let owner_id = auth_user.user_id;

    let shares: Vec<(VehicleShare, String, String, String, String)> = conn
        .interact(move |conn| {
            vehicle_shares::table
                .inner_join(vehicles::table)
                .inner_join(users::table.on(vehicle_shares::shared_with_user_id.eq(users::id)))
                .filter(vehicles::owner_id.eq(owner_id))
                .select((
                    VehicleShare::as_select(),
                    vehicles::make,
                    vehicles::model,
                    vehicles::registration,
                    users::email,
                ))
                .order(vehicle_shares::created_at.desc())
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let response: Vec<OwnedVehicleShareResponse> = shares
        .into_iter()
        .map(
            |(share, make, model, registration, shared_with_email)| OwnedVehicleShareResponse {
                id: share.id,
                vehicle_id: share.vehicle_id,
                vehicle_make: make,
                vehicle_model: model,
                vehicle_registration: registration,
                shared_with_email,
                permission_level: share.permission_level,
                created_at: share.created_at,
            },
        )
        .collect();

    Ok(Json(response))
}

#[derive(serde::Deserialize)]
pub struct CreateShareRequest {
    pub vehicle_id: i32,
    pub shared_with_email: String,
    pub permission_level: String,
}

pub async fn create_share(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let owner_id = auth_user.user_id;

    // Validate vehicle ownership
    let vehicle_exists: bool = conn
        .interact(move |conn| {
            vehicles::table
                .filter(vehicles::id.eq(payload.vehicle_id))
                .filter(vehicles::owner_id.eq(owner_id))
                .first::<crate::models::Vehicle>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?
        .map(|_| true)
        .unwrap_or(false);

    if !vehicle_exists {
        return Err((
            StatusCode::FORBIDDEN,
            "You don't own this vehicle".to_string(),
        ));
    }

    // Find recipient user by email
    let recipient_email = payload.shared_with_email.clone();
    let recipient: Option<User> = conn
        .interact(move |conn| {
            users::table
                .filter(users::email.eq(&recipient_email))
                .first(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let recipient = match recipient {
        Some(user) => user,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("User with email '{}' not found", payload.shared_with_email),
            ));
        }
    };

    // Don't allow sharing with yourself
    if recipient.id == owner_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "Cannot share a vehicle with yourself".to_string(),
        ));
    }

    // Validate permission level
    let permission_level = payload.permission_level.to_lowercase();
    if !matches!(permission_level.as_str(), "read" | "write") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Permission level must be 'read' or 'write'".to_string(),
        ));
    }

    let vehicle_id = payload.vehicle_id;
    let shared_with_user_id = recipient.id;
    let permission_level_clone = permission_level.clone();

    // Check if share already exists
    let existing_share: Option<VehicleShare> = conn
        .interact(move |conn| {
            vehicle_shares::table
                .filter(vehicle_shares::vehicle_id.eq(vehicle_id))
                .filter(vehicle_shares::shared_with_user_id.eq(shared_with_user_id))
                .first(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

    let share = if let Some(existing) = existing_share {
        // Update existing share
        conn.interact(move |conn| {
            diesel::update(vehicle_shares::table.filter(vehicle_shares::id.eq(existing.id)))
                .set(vehicle_shares::permission_level.eq(permission_level_clone))
                .returning(VehicleShare::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?
    } else {
        // Insert new share
        conn.interact(move |conn| {
            diesel::insert_into(vehicle_shares::table)
                .values(NewVehicleShare {
                    vehicle_id,
                    shared_with_user_id,
                    permission_level: Some(permission_level),
                })
                .returning(VehicleShare::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?
    };

    Ok((StatusCode::CREATED, Json(share)))
}

pub async fn delete_share(
    State(pool): State<Pool>,
    headers: HeaderMap,
    axum::extract::Path(share_id): axum::extract::Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let user_id = auth_user.user_id;

    // Check if user owns the vehicle that this share belongs to
    let can_delete: bool = conn
        .interact(move |conn| {
            vehicle_shares::table
                .inner_join(vehicles::table)
                .filter(vehicle_shares::id.eq(share_id))
                .filter(vehicles::owner_id.eq(user_id))
                .select(vehicle_shares::id)
                .first::<i32>(conn)
                .optional()
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?
        .map(|_| true)
        .unwrap_or(false);

    if !can_delete {
        return Err((
            StatusCode::FORBIDDEN,
            "You don't have permission to delete this share".to_string(),
        ));
    }

    conn.interact(move |conn| {
        diesel::delete(vehicle_shares::table.filter(vehicle_shares::id.eq(share_id))).execute(conn)
    })
    .await
    .map_err(internal_error)?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}
