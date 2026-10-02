use axum::http::StatusCode;
use diesel::prelude::*;

mod common;

use common::AuthenticatedRequest;

async fn station_name(pool: &deadpool_diesel::postgres::Pool, station_id: i32) -> Option<String> {
    let conn = pool.get().await.unwrap();
    conn.interact(move |conn| {
        deesl::schema::fuel_stations::table
            .filter(deesl::schema::fuel_stations::id.eq(station_id))
            .select(deesl::schema::fuel_stations::name)
            .first::<String>(conn)
            .optional()
    })
    .await
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn test_stations_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "stations_page").await;

    let response = env.server.get("/stations").with_auth(&user.token).await;

    response.assert_status_ok();
    assert!(response.text().contains("Station Manager"));
}

#[tokio::test]
async fn test_create_station_redirects() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "station_create").await;

    let response = env
        .server
        .post("/stations")
        .with_auth(&user.token)
        .form(&[("name", "Shell")])
        .await;

    common::assert_hx_redirect(&response, "/stations");

    let conn = env.pool.get().await.unwrap();
    let user_id = user.id;
    let exists: bool = conn
        .interact(move |conn| {
            deesl::schema::fuel_stations::table
                .filter(deesl::schema::fuel_stations::user_id.eq(user_id))
                .filter(deesl::schema::fuel_stations::name.eq("Shell"))
                .first::<deesl::models::FuelStation>(conn)
                .optional()
                .map(|s| s.is_some())
        })
        .await
        .unwrap()
        .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn test_update_station_updates_name() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "station_update").await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Old Name").await;

    let response = env
        .server
        .post(&format!("/stations/{}", station_id))
        .with_auth(&user.token)
        .form(&[("name", "New Name")])
        .await;

    common::assert_hx_redirect(&response, "/stations");
    assert_eq!(
        station_name(&env.pool, station_id).await.as_deref(),
        Some("New Name")
    );
}

#[tokio::test]
async fn test_update_station_when_not_owner_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "station_update_owner").await;
    let other = common::create_test_user(&env, "station_update_other").await;
    let station_id = common::create_test_station_db(&env.pool, owner.id, "Owned").await;

    let response = env
        .server
        .post(&format!("/stations/{}", station_id))
        .with_auth(&other.token)
        .form(&[("name", "Hijacked")])
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
    assert_eq!(
        station_name(&env.pool, station_id).await.as_deref(),
        Some("Owned")
    );
}

#[tokio::test]
async fn test_delete_station_removes_station() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "station_delete").await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Doomed").await;

    let response = env
        .server
        .delete(&format!("/stations/{}", station_id))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert_eq!(response.text(), "");
    assert_eq!(station_name(&env.pool, station_id).await, None);
}

#[tokio::test]
async fn test_delete_station_when_not_owner_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "station_delete_owner").await;
    let other = common::create_test_user(&env, "station_delete_other").await;
    let station_id = common::create_test_station_db(&env.pool, owner.id, "Safe").await;

    let response = env
        .server
        .delete(&format!("/stations/{}", station_id))
        .with_auth(&other.token)
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
    assert!(station_name(&env.pool, station_id).await.is_some());
}

#[tokio::test]
async fn test_merge_stations() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "merge_test").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Merge", "Car", "MERGE-1").await;

    let station1_id = common::create_test_station_db(&env.pool, user.id, "Station 1").await;
    let station2_id = common::create_test_station_db(&env.pool, user.id, "Station 2").await;

    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        Some(station1_id),
        100,
        10.0,
        15.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let form = [("target_id", station2_id.to_string())];
    let response = env
        .server
        .post(&format!("/stations/{}/merge", station1_id))
        .with_auth(&user.token)
        .form(&form)
        .await;

    common::assert_hx_redirect(&response, "/stations");
    assert_eq!(station_name(&env.pool, station1_id).await, None);

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

#[tokio::test]
async fn test_merge_stations_when_source_equals_target_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "merge_self").await;
    let station_id = common::create_test_station_db(&env.pool, user.id, "Self").await;

    let response = env
        .server
        .post(&format!("/stations/{}/merge", station_id))
        .with_auth(&user.token)
        .form(&[("target_id", station_id.to_string())])
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_merge_stations_when_source_not_owned_is_internal_error() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "merge_source_owner").await;
    let other = common::create_test_user(&env, "merge_source_other").await;
    let source_id = common::create_test_station_db(&env.pool, owner.id, "Foreign").await;
    let target_id = common::create_test_station_db(&env.pool, other.id, "Target").await;

    let response = env
        .server
        .post(&format!("/stations/{}/merge", source_id))
        .with_auth(&other.token)
        .form(&[("target_id", target_id.to_string())])
        .await;

    response.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_merge_stations_when_target_missing_is_internal_error() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "merge_target_missing").await;
    let source_id = common::create_test_station_db(&env.pool, user.id, "Source").await;

    let response = env
        .server
        .post(&format!("/stations/{}/merge", source_id))
        .with_auth(&user.token)
        .form(&[("target_id", "999999")])
        .await;

    response.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_htmx_station_search_when_query_too_short_returns_empty() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "search_short").await;
    common::create_test_station_db(&env.pool, user.id, "Shell").await;

    let response = env
        .server
        .get("/stations/htmx/search?q=S")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert_eq!(response.text(), "");
}

#[tokio::test]
async fn test_htmx_station_search_matches_name() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "search_match").await;
    common::create_test_station_db(&env.pool, user.id, "Shell Station").await;

    let response = env
        .server
        .get("/stations/htmx/search?q=Shell")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("Shell Station"));
}

#[tokio::test]
async fn test_htmx_station_search_matches_fuzzy() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "search_fuzzy").await;
    common::create_test_station_db(&env.pool, user.id, "Shell").await;

    let response = env
        .server
        .get("/stations/htmx/search?q=shl")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("Shell"));
}

#[tokio::test]
async fn test_htmx_station_search_excludes_other_users_stations() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "search_owner").await;
    let other = common::create_test_user(&env, "search_other").await;
    common::create_test_station_db(&env.pool, other.id, "SecretFuel").await;

    let response = env
        .server
        .get("/stations/htmx/search?q=SecretFuel")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("No stations found"));
}

#[tokio::test]
async fn test_htmx_station_search_includes_global_stations() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "search_global").await;
    common::create_test_global_station_db(&env.pool, "GlobalGas").await;

    let response = env
        .server
        .get("/stations/htmx/search?q=GlobalGas")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("GlobalGas"));
}
