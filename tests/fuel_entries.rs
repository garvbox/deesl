use axum::http::StatusCode;
use diesel::prelude::*;

mod common;

use common::AuthenticatedRequest;

async fn entry_mileage(pool: &deadpool_diesel::postgres::Pool, entry_id: i32) -> Option<i32> {
    let conn = pool.get().await.unwrap();
    conn.interact(move |conn| {
        deesl::schema::fuel_entries::table
            .filter(deesl::schema::fuel_entries::id.eq(entry_id))
            .select(deesl::schema::fuel_entries::mileage_km)
            .first::<i32>(conn)
            .optional()
    })
    .await
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn test_fuel_entries_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "entries_page").await;

    let response = env.server.get("/fuel-entries").with_auth(&user.token).await;

    response.assert_status_ok();
    assert!(response.text().contains("Fuel Log"));
}

#[tokio::test]
async fn test_new_fuel_entry_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "new_entry_page").await;

    let response = env
        .server
        .get("/fuel-entries/new")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("Add Fuel Entry"));
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

    common::assert_hx_redirect(&response, "/dashboard");
}

#[tokio::test]
async fn test_create_fuel_entry_when_no_access_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "create_no_access_owner").await;
    let other = common::create_test_user(&env, "create_no_access_other").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, owner.id, "Private", "Car", "PRIV-1").await;
    let station_id = common::create_test_station_db(&env.pool, owner.id, "Station").await;

    let form = [
        ("vehicle_id", vehicle_id.to_string()),
        ("station_id", station_id.to_string()),
        ("mileage_km", "100".to_string()),
        ("litres", "10.0".to_string()),
        ("cost", "20.0".to_string()),
        ("filled_at", "2024-03-01T12:00".to_string()),
    ];

    let response = env
        .server
        .post("/fuel-entries")
        .with_auth(&other.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_fuel_entry_with_shared_write_access() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "owner").await;
    let shared_user = common::create_test_user(&env, "shared").await;

    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, owner.id, "Shared", "Car", "SHARE-1").await;
    let station_id = common::create_test_station_db(&env.pool, owner.id, "Test Station").await;

    common::create_test_vehicle_share_db(&env.pool, vehicle_id, shared_user.id, "write").await;

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
        .with_auth(&shared_user.token)
        .form(&form)
        .await;

    common::assert_hx_redirect(&response, "/dashboard");
}

#[tokio::test]
async fn test_create_fuel_entry_with_shared_read_access_denied() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "owner").await;
    let shared_user = common::create_test_user(&env, "shared").await;

    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, owner.id, "Shared", "Car", "SHARE-2").await;
    let station_id = common::create_test_station_db(&env.pool, owner.id, "Test Station").await;

    common::create_test_vehicle_share_db(&env.pool, vehicle_id, shared_user.id, "read").await;

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
        .with_auth(&shared_user.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_fuel_entry_when_fields_missing_is_unprocessable() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "create_entry_missing").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;

    let response = env
        .server
        .post("/fuel-entries")
        .with_auth(&user.token)
        .form(&[("vehicle_id", vehicle_id.to_string())])
        .await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_fuel_entry_when_filled_at_invalid_uses_default() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "entry_bad_date").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Station").await;

    let form = [
        ("vehicle_id", vehicle_id.to_string()),
        ("station_id", station_id.to_string()),
        ("mileage_km", "123".to_string()),
        ("litres", "10.0".to_string()),
        ("cost", "20.0".to_string()),
        ("filled_at", "not-a-date".to_string()),
    ];

    let response = env
        .server
        .post("/fuel-entries")
        .with_auth(&user.token)
        .form(&form)
        .await;

    common::assert_hx_redirect(&response, "/dashboard");
}

#[tokio::test]
async fn test_fuel_entries_page_filters_by_vehicle() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "filter_vehicle").await;
    let v1 = common::create_test_vehicle_db(&env.pool, user.id, "Filter", "One", "FILT-A").await;
    let v2 = common::create_test_vehicle_db(&env.pool, user.id, "Filter", "Two", "FILT-B").await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Station").await;

    common::create_test_entry_db(
        &env.pool,
        v1,
        Some(station_id),
        11111,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;
    common::create_test_entry_db(
        &env.pool,
        v2,
        Some(station_id),
        22222,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .get(&format!("/fuel-entries?vehicle_id={}", v1))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("11111"));
    assert!(!body.contains("22222"));
}

#[tokio::test]
async fn test_fuel_entries_page_filters_by_station() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "filter_station").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let s1 = common::create_test_station_db(&env.pool, user.id, "Alpha").await;
    let s2 = common::create_test_station_db(&env.pool, user.id, "Beta").await;

    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        Some(s1),
        11111,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;
    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        Some(s2),
        22222,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .get(&format!("/fuel-entries?station_id={}", s1))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("11111"));
    assert!(!body.contains("22222"));
}

#[tokio::test]
async fn test_fuel_entries_page_filters_by_date_range() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "filter_date").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Station").await;

    let early = chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let late = chrono::NaiveDate::from_ymd_opt(2024, 6, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();

    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        Some(station_id),
        11111,
        10.0,
        20.0,
        early,
    )
    .await;
    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        Some(station_id),
        22222,
        10.0,
        20.0,
        late,
    )
    .await;

    let response = env
        .server
        .get("/fuel-entries?date_from=2024-05-01&date_to=2024-12-31")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("22222"));
    assert!(!body.contains("11111"));
}

#[tokio::test]
async fn test_htmx_recent_entries_returns_fragment() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "recent_test").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Audi", "A4", "AUDI-44").await;

    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        45.6,
        78.9,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .get("/fuel-entries/htmx/recent")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("AUDI-44"));
    assert!(response.text().contains("45.60 L"));
    assert!(response.text().contains("€78.90"));
}

#[tokio::test]
async fn test_htmx_recent_entries_excludes_other_users_entries() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "recent_owner").await;
    let other = common::create_test_user(&env, "recent_other").await;

    let own_vehicle =
        common::create_test_vehicle_db(&env.pool, user.id, "Mine", "Car", "RECENT-MINE").await;
    let other_vehicle =
        common::create_test_vehicle_db(&env.pool, other.id, "Theirs", "Car", "RECENT-THEIRS").await;

    common::create_test_entry_db(
        &env.pool,
        own_vehicle,
        None,
        100,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;
    common::create_test_entry_db(
        &env.pool,
        other_vehicle,
        None,
        200,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .get("/fuel-entries/htmx/recent")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("RECENT-MINE"));
    assert!(!response.text().contains("RECENT-THEIRS"));
}

#[tokio::test]
async fn test_edit_fuel_entry_page_loads_for_owner() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "edit_entry_owner").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .get(&format!("/fuel-entries/{}/edit", entry_id))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("Edit Fuel Entry"));
}

#[tokio::test]
async fn test_edit_fuel_entry_when_not_owner_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "edit_entry_owner2").await;
    let other = common::create_test_user(&env, "edit_entry_other").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, owner.id).await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .get(&format!("/fuel-entries/{}/edit", entry_id))
        .with_auth(&other.token)
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_edit_fuel_entry_when_missing_is_not_found() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "edit_entry_missing").await;

    let response = env
        .server
        .get("/fuel-entries/999999/edit")
        .with_auth(&user.token)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_fuel_entry_persists_changes() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "update_entry_owner").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Station").await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let form = [
        ("station_id", station_id.to_string()),
        ("mileage_km", "54321".to_string()),
        ("litres", "30.0".to_string()),
        ("cost", "45.0".to_string()),
        ("filled_at", "2024-03-01T12:00".to_string()),
    ];

    let response = env
        .server
        .post(&format!("/fuel-entries/{}", entry_id))
        .with_auth(&user.token)
        .form(&form)
        .await;

    common::assert_hx_redirect(&response, "/dashboard");
    assert_eq!(entry_mileage(&env.pool, entry_id).await, Some(54321));
}

#[tokio::test]
async fn test_update_fuel_entry_when_not_owner_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "update_entry_owner2").await;
    let other = common::create_test_user(&env, "update_entry_other").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, owner.id).await;
    let station_id = common::create_test_station_db(&env.pool, owner.id, "Station").await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let form = [
        ("station_id", station_id.to_string()),
        ("mileage_km", "54321".to_string()),
        ("litres", "30.0".to_string()),
        ("cost", "45.0".to_string()),
        ("filled_at", "2024-03-01T12:00".to_string()),
    ];

    let response = env
        .server
        .post(&format!("/fuel-entries/{}", entry_id))
        .with_auth(&other.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(entry_mileage(&env.pool, entry_id).await, Some(12345));
}

#[tokio::test]
async fn test_update_fuel_entry_when_missing_is_not_found() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "update_entry_missing").await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Station").await;

    let form = [
        ("station_id", station_id.to_string()),
        ("mileage_km", "54321".to_string()),
        ("litres", "30.0".to_string()),
        ("cost", "45.0".to_string()),
        ("filled_at", "2024-03-01T12:00".to_string()),
    ];

    let response = env
        .server
        .post("/fuel-entries/999999")
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_fuel_entry_when_invalid_date_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "update_entry_bad_date").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Station").await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let form = [
        ("station_id", station_id.to_string()),
        ("mileage_km", "54321".to_string()),
        ("litres", "30.0".to_string()),
        ("cost", "45.0".to_string()),
        ("filled_at", "not-a-date".to_string()),
    ];

    let response = env
        .server
        .post(&format!("/fuel-entries/{}", entry_id))
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_htmx_delete_fuel_entry() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "delete_entry_owner").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .delete(&format!("/fuel-entries/htmx/{}", entry_id))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert_eq!(response.text(), "");
    assert_eq!(entry_mileage(&env.pool, entry_id).await, None);
}

#[tokio::test]
async fn test_htmx_delete_fuel_entry_when_not_owner_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "delete_entry_owner2").await;
    let other = common::create_test_user(&env, "delete_entry_other").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, owner.id).await;
    let entry_id = common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        12345,
        10.0,
        20.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env
        .server
        .delete(&format!("/fuel-entries/htmx/{}", entry_id))
        .with_auth(&other.token)
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
    assert!(entry_mileage(&env.pool, entry_id).await.is_some());
}

#[tokio::test]
async fn test_htmx_delete_fuel_entry_when_missing_is_not_found() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "delete_entry_missing").await;

    let response = env
        .server
        .delete("/fuel-entries/htmx/999999")
        .with_auth(&user.token)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}
