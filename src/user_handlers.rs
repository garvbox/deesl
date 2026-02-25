use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::extract_auth_user;
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

pub async fn get_me(
    State(pool): State<Pool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;

    let user: User = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(auth_user.user_id))
                .first(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    Ok(Json(UserProfileResponse::from(user)))
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub currency: String,
}

pub(crate) const SUPPORTED_CURRENCIES: &[&str] = &["EUR", "GBP", "USD", "CAD", "AUD"];

pub(crate) fn validate_currency(currency: &str) -> Result<(), (StatusCode, String)> {
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

pub async fn update_me(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    validate_currency(&payload.currency)?;

    let auth_user = extract_auth_user(&headers)?;
    let conn = pool.get().await.map_err(internal_error)?;
    let currency = payload.currency.clone();

    let user: User = conn
        .interact(move |conn| {
            diesel::update(users::table.filter(users::id.eq(auth_user.user_id)))
                .set(users::currency.eq(currency))
                .returning(User::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    Ok(Json(UserProfileResponse::from(user)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::User;

    fn make_user(id: i32, email: &str, currency: &str) -> User {
        User {
            id,
            email: email.to_string(),
            password_hash: None,
            created_at: chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc(),
            currency: currency.to_string(),
            google_id: None,
        }
    }

    // --- validate_currency ---

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

    #[test]
    fn test_validate_currency_rejects_empty_string() {
        let result = validate_currency("");
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_currency_rejects_lowercase() {
        assert!(validate_currency("eur").is_err());
        assert!(validate_currency("usd").is_err());
    }

    #[test]
    fn test_validate_currency_error_message_lists_supported_currencies() {
        let (_, msg) = validate_currency("JPY").unwrap_err();
        for currency in SUPPORTED_CURRENCIES {
            assert!(msg.contains(currency), "message should list {currency}");
        }
    }

    // --- UserProfileResponse::from ---

    #[test]
    fn test_user_profile_response_maps_fields_correctly() {
        let user = make_user(7, "test@example.com", "GBP");
        let response = UserProfileResponse::from(user);
        assert_eq!(response.id, 7);
        assert_eq!(response.email, "test@example.com");
        assert_eq!(response.currency, "GBP");
    }

    #[test]
    fn test_user_profile_response_from_user_with_google_id() {
        let mut user = make_user(3, "google@example.com", "USD");
        user.google_id = Some("google-sub-123".to_string());
        let response = UserProfileResponse::from(user);
        assert_eq!(response.id, 3);
        assert_eq!(response.email, "google@example.com");
        assert_eq!(response.currency, "USD");
    }
}
