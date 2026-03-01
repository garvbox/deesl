use axum::http::StatusCode;
use pretty_assertions::assert_eq;

mod common;

use common::AuthenticatedRequest;

#[tokio::test]
async fn test_root_redirects_to_dashboard() {
    let env = common::create_test_env().await;
    let response = env.server.get("/").await;

    // Root should redirect to /dashboard
    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("location"), "/dashboard");
}

#[tokio::test]
async fn test_dashboard_requires_auth_and_redirects() {
    let env = common::create_test_env().await;

    // Request without auth
    let response = env.server.get("/dashboard").await;

    // AuthUserRedirect should redirect to /login
    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("location"), "/login");
}

#[tokio::test]
async fn test_dashboard_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "dashboard_test").await;

    let response = env.server.get("/dashboard").with_auth(&user.token).await;

    response.assert_status_ok();
    // Verify it's an HTML response and contains key dashboard elements
    assert_eq!(response.content_type(), "text/html; charset=utf-8");
    assert!(response.text().contains("Dashboard"));
    assert!(response.text().contains("Your Vehicles"));
}

#[tokio::test]
async fn test_htmx_vehicles_returns_fragment() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "htmx_test").await;

    // Create a vehicle first
    common::create_test_vehicle_db(&env.pool, user.id, "Tesla", "Model 3", "HTMX-123").await;

    let response = env
        .server
        .get("/htmx/vehicles")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    // Fragments don't have the full layout, just the list content
    assert!(response.text().contains("Tesla Model 3"));
    assert!(response.text().contains("HTMX-123"));
}

#[tokio::test]
async fn test_create_vehicle_and_redirects() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "create_test").await;

    let form = [
        ("make", "BMW"),
        ("model", "M3"),
        ("registration", "M3-RACE"),
    ];

    let response = env
        .server
        .post("/vehicles")
        .with_auth(&user.token)
        .form(&form)
        .await;

    // Should return 303 Redirect back to dashboard (via HX-Redirect or standard Location)
    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("HX-Redirect"), "/dashboard");
}

#[tokio::test]
async fn test_settings_can_update_currency() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "settings_test").await;

    let form = [("currency", "USD")];

    let response = env
        .server
        .patch("/settings")
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("successfully"));
}

#[tokio::test]
async fn test_import_preview_accepts_csv() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "import_test").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Import", "Car", "IMP-1").await;

    let csv_content = b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000";

    let response = common::post_import_csv(
        &env.server,
        "/htmx/import/preview",
        &user.token,
        vehicle_id,
        csv_content,
    )
    .await;

    response.assert_status_ok();
    assert!(response.text().contains("Map CSV Columns"));
    assert!(response.text().contains("2024-03-01"));
}
