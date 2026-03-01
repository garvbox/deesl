use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;

use crate::AppState;
use crate::auth::{AuthUser, AuthUserRedirect};
use crate::models::Vehicle;
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
    LoginTemplate { logged_in: false }
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub logged_in: bool,
    pub user_email: String,
}

pub async fn dashboard(AuthUserRedirect(user): AuthUserRedirect) -> impl IntoResponse {
    DashboardTemplate {
        logged_in: true,
        user_email: user.email,
    }
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
        html.push_str(&format!(
            r#"<div style="padding: 1rem; border: 1px solid var(--border); border-radius: 8px;">
                <strong>{} {}</strong> <span style="color: var(--text-muted); font-size: 0.8rem;">{}</span>
            </div>"#,
            v.make, v.model, v.registration
        ));
    }
    html.push_str("</div>");

    Ok(Html(html))
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

    #[test]
    fn test_internal_error_returns_error_message_as_body() {
        let (_, body) = internal_error(TestError("something went wrong".to_string()));
        assert_eq!(body, "something went wrong");
    }

    #[test]
    fn test_internal_error_with_empty_message() {
        let (status, body) = internal_error(TestError(String::new()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, "");
    }
}
