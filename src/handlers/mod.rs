use axum::{
    http::HeaderMap,
    response::{IntoResponse, Redirect},
};
use diesel::prelude::*;

use crate::error::AppError;
use crate::models::Vehicle;
use crate::schema::vehicles as vehicles_schema;

pub struct HxRedirect(pub &'static str);

impl IntoResponse for HxRedirect {
    fn into_response(self) -> axum::response::Response {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Redirect", self.0.parse().unwrap());
        (headers, Redirect::to(self.0)).into_response()
    }
}

pub const SUPPORTED_CURRENCIES: &[&str] = &["EUR", "GBP", "USD", "CAD", "AUD"];

pub fn validate_currency(currency: &str) -> Result<(), AppError> {
    if SUPPORTED_CURRENCIES.contains(&currency) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "Unsupported currency '{}'. Must be one of: {}",
            currency,
            SUPPORTED_CURRENCIES.join(", ")
        )))
    }
}

pub async fn check_vehicle_write_access(
    conn: &deadpool_diesel::postgres::Object,
    user_id: i32,
    vehicle_id: i32,
) -> Result<(), AppError> {
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
        .await??;

    let (has_access, has_write) = has_write_access;

    if !has_access {
        return Err(AppError::Forbidden(
            "You don't have access to this vehicle".to_string(),
        ));
    }

    if !has_write {
        return Err(AppError::Forbidden(
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
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("XYZ"));
        assert!(msg.contains("Must be one of"));
    }
}
