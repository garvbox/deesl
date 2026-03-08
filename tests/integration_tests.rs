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
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Audi", "A4", "AUDI-44").await;

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
    })
    .await
    .unwrap()
    .unwrap();

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
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Ford", "Focus", "FORD-F").await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Test Station").await;

    let form = [
        ("vehicle_id", vehicle_id.to_string()),
        ("station_id", station_id.to_string()),
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
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Old", "Car", "OLD-1").await;

    let response = env
        .server
        .delete(&format!("/htmx/vehicles/{}", vehicle_id))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert_eq!(response.text(), "");

    // Verify it's gone from DB
    let conn = env.pool.get().await.unwrap();
    let exists: bool = conn
        .interact(move |conn| {
            deesl::schema::vehicles::table
                .filter(deesl::schema::vehicles::id.eq(vehicle_id))
                .first::<deesl::models::Vehicle>(conn)
                .optional()
                .map(|v| v.is_some())
        })
        .await
        .unwrap()
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn test_htmx_import_execute() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "import_exec").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Import", "Car", "IMP-2").await;

    let csv_content = b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000";

    // Step 1: Call preview to store CSV and get import_id
    let preview_response = common::post_import_csv(
        &env.server,
        "/htmx/import/preview",
        &user.token,
        vehicle_id,
        csv_content,
        None,
    )
    .await;

    preview_response.assert_status_ok();
    let preview_html = preview_response.text();

    // Extract import_id from the HTML (it's in a hidden input field)
    let import_id = preview_html
        .split("name=\"import_id\" value=\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    // Step 2: Call execute with import_id and mappings
    let mut mappings = std::collections::HashMap::new();
    mappings.insert("col_0".to_string(), "Date".to_string());
    mappings.insert("map_0".to_string(), "filled_at_date".to_string());
    mappings.insert("col_1".to_string(), "Litres".to_string());
    mappings.insert("map_1".to_string(), "litres".to_string());
    mappings.insert("col_2".to_string(), "Cost".to_string());
    mappings.insert("map_2".to_string(), "cost".to_string());
    mappings.insert("col_3".to_string(), "Mileage".to_string());
    mappings.insert("map_3".to_string(), "mileage_km".to_string());

    let response =
        common::post_import_execute(&env.server, &user.token, import_id, vehicle_id, mappings)
            .await;

    response.assert_status_ok();
    assert!(response.text().contains("Import Successful"));
    assert!(response.text().contains("1")); // 1 imported
}

#[tokio::test]
async fn test_merge_stations() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "merge_test").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Merge", "Car", "MERGE-1").await;

    // Create two stations
    let station1_id = common::create_test_station_db(&env.pool, user.id, "Station 1").await;
    let station2_id = common::create_test_station_db(&env.pool, user.id, "Station 2").await;

    // Create a fuel entry for station 1
    let conn = env.pool.get().await.unwrap();
    conn.interact(move |conn| {
        diesel::insert_into(deesl::schema::fuel_entries::table)
            .values((
                deesl::schema::fuel_entries::vehicle_id.eq(vehicle_id),
                deesl::schema::fuel_entries::station_id.eq(Some(station1_id)),
                deesl::schema::fuel_entries::mileage_km.eq(100),
                deesl::schema::fuel_entries::litres.eq(10.0),
                deesl::schema::fuel_entries::cost.eq(15.0),
                deesl::schema::fuel_entries::filled_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
    })
    .await
    .unwrap()
    .unwrap();

    // Merge station 1 into station 2
    let form = [("target_id", station2_id.to_string())];
    let response = env
        .server
        .post(&format!("/stations/{}/merge", station1_id))
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("HX-Redirect"), "/stations");

    // Verify station 1 is deleted
    let conn = env.pool.get().await.unwrap();
    let s1_exists: bool = conn
        .interact(move |conn| {
            deesl::schema::fuel_stations::table
                .filter(deesl::schema::fuel_stations::id.eq(station1_id))
                .first::<deesl::models::FuelStation>(conn)
                .optional()
                .map(|s| s.is_some())
        })
        .await
        .unwrap()
        .unwrap();
    assert!(!s1_exists);

    // Verify entry now points to station 2
    let conn = env.pool.get().await.unwrap();
    let entry_station_id: Option<i32> = conn
        .interact(move |conn| {
            deesl::schema::fuel_entries::table
                .filter(deesl::schema::fuel_entries::vehicle_id.eq(vehicle_id))
                .select(deesl::schema::fuel_entries::station_id)
                .first::<Option<i32>>(conn)
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry_station_id, Some(station2_id));
}

