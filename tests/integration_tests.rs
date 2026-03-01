use axum::http::StatusCode;
use diesel::prelude::*;
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
        None,
    )
    .await;

    response.assert_status_ok();
    assert!(response.text().contains("Map CSV Columns"));
    assert!(response.text().contains("2024-03-01"));
}

#[tokio::test]
async fn test_htmx_recent_entries_returns_fragment() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "recent_test").await;
    let vehicle_id = common::create_test_vehicle_db(&env.pool, user.id, "Audi", "A4", "AUDI-44").await;
    
    // Create a fuel entry in DB
    let conn = env.pool.get().await.unwrap();
    conn.interact(move |conn| {
        diesel::insert_into(deesl::schema::fuel_entries::table)
            .values((
                deesl::schema::fuel_entries::vehicle_id.eq(vehicle_id),
                deesl::schema::fuel_entries::mileage_km.eq(12345),
                deesl::schema::fuel_entries::litres.eq(45.6),
                deesl::schema::fuel_entries::cost.eq(78.9),
                deesl::schema::fuel_entries::filled_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
    }).await.unwrap().unwrap();

    let response = env
        .server
        .get("/htmx/entries/recent")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("AUDI-44"));
    assert!(response.text().contains("45.60 L"));
    assert!(response.text().contains("€78.90"));
}

#[tokio::test]
async fn test_create_fuel_entry_and_redirects() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "fuel_test").await;
    let vehicle_id = common::create_test_vehicle_db(&env.pool, user.id, "Ford", "Focus", "FORD-F").await;

    let form = [
        ("vehicle_id", vehicle_id.to_string()),
        ("mileage_km", "100500".to_string()),
        ("litres", "50.0".to_string()),
        ("cost", "80.0".to_string()),
        ("filled_at", "2024-03-01T12:00".to_string()),
    ];

    let response = env
        .server
        .post("/fuel-entries")
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("HX-Redirect"), "/dashboard");
}

#[tokio::test]
async fn test_htmx_delete_vehicle() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "delete_test").await;
    let vehicle_id = common::create_test_vehicle_db(&env.pool, user.id, "Old", "Car", "OLD-1").await;

    let response = env
        .server
        .delete(&format!("/htmx/vehicles/{}", vehicle_id))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert_eq!(response.text(), "");

    // Verify it's gone from DB
    let conn = env.pool.get().await.unwrap();
    let exists: bool = conn.interact(move |conn| {
        deesl::schema::vehicles::table
            .filter(deesl::schema::vehicles::id.eq(vehicle_id))
            .first::<deesl::models::Vehicle>(conn)
            .optional()
            .map(|v| v.is_some())
    }).await.unwrap().unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn test_htmx_import_execute() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "import_exec").await;
    let vehicle_id = common::create_test_vehicle_db(&env.pool, user.id, "Import", "Car", "IMP-2").await;

    let csv_content = b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000";
    let mut mappings = std::collections::HashMap::new();
    mappings.insert("col_0".to_string(), "Date".to_string());
    mappings.insert("map_0".to_string(), "filled_at_date".to_string());
    mappings.insert("col_1".to_string(), "Litres".to_string());
    mappings.insert("map_1".to_string(), "litres".to_string());
    mappings.insert("col_2".to_string(), "Cost".to_string());
    mappings.insert("map_2".to_string(), "cost".to_string());
    mappings.insert("col_3".to_string(), "Mileage".to_string());
    mappings.insert("map_3".to_string(), "mileage_km".to_string());

    let response = common::post_import_csv(
        &env.server,
        "/htmx/import/execute",
        &user.token,
        vehicle_id,
        csv_content,
        Some(mappings),
    )
    .await;

    response.assert_status_ok();
    assert!(response.text().contains("Import Successful"));
    assert!(response.text().contains("1")); // 1 imported
}
