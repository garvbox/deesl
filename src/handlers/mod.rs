use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
};
use diesel::prelude::*;

use crate::models::Vehicle;
use crate::schema::vehicles as vehicles_schema;

/// Utility function for mapping any error into a `500 Internal Server Error`
pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::fmt::Display,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub struct HxRedirect(pub &'static str);

impl IntoResponse for HxRedirect {
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Redirect", self.0.parse().unwrap());
        (headers, Redirect::to(self.0)).into_response()
    }
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

pub async fn check_vehicle_write_access(
    conn: &deadpool_diesel::postgres::Object,
    user_id: i32,
    vehicle_id: i32,
) -> Result<(), (StatusCode, String)> {
    let has_write_access: (bool, bool) = conn
        .interact(move |conn| {
            let vehicle: Vehicle = vehicles_schema::table
                .filter(vehicles_schema::id.eq(vehicle_id))
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

/// Simple fuzzy matching - checks if all characters in query appear in name in order
pub fn fuzzy_match(name: &str, query: &str) -> bool {
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

// Module declarations will be added here as we move handlers
pub mod auth;
pub mod fuel_entries;
pub mod import;
pub mod misc;
pub mod settings;
pub mod stations;
pub mod stats;
pub mod vehicles;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestError(String);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
