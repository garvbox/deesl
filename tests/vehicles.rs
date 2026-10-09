use axum::http::StatusCode;
use diesel::prelude::*;

mod common;

use common::AuthenticatedRequest;

async fn vehicle_exists(pool: &deadpool_diesel::postgres::Pool, vehicle_id: i32) -> bool {
    let conn = pool.get().await.unwrap();
    conn.interact(move |conn| {
        deesl::schema::vehicles::table
            .filter(deesl::schema::vehicles::id.eq(vehicle_id))
            .first::<deesl::models::Vehicle>(conn)
            .optional()
            .map(|v| v.is_some())
    })
    .await
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn test_vehicles_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "vehicles_page").await;

    let response = env.server.get("/vehicles").with_auth(&user.token).await;

    response.assert_status_ok();
    assert!(response.text().contains("My Vehicles"));
}

#[tokio::test]
async fn test_new_vehicle_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "new_vehicle_page").await;

    let response = env.server.get("/vehicles/new").with_auth(&user.token).await;

    response.assert_status_ok();
    assert!(response.text().contains("Add Vehicle"));
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

    common::assert_hx_redirect(&response, "/dashboard");
}

#[tokio::test]
async fn test_create_vehicle_when_fields_missing_is_unprocessable() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "create_missing").await;

    let response = env
        .server
        .post("/vehicles")
        .with_auth(&user.token)
        .form(&[("make", "BMW")])
        .await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_htmx_vehicles_returns_fragment() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "htmx_test").await;

    common::create_test_vehicle_db(&env.pool, user.id, "Tesla", "Model 3", "HTMX-123").await;

    let response = env
        .server
        .get("/vehicles/htmx/list")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("Tesla Model 3"));
    assert!(response.text().contains("HTMX-123"));
}

#[tokio::test]
async fn test_htmx_vehicles_excludes_other_users_vehicles() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "list_owner").await;
    let other = common::create_test_user(&env, "list_other").await;

    common::create_test_vehicle_db(&env.pool, user.id, "Mine", "Car", "MINE-1").await;
    common::create_test_vehicle_db(&env.pool, other.id, "Theirs", "Car", "THEIRS-1").await;

    let response = env
        .server
        .get("/vehicles/htmx/list")
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert!(response.text().contains("MINE-1"));
    assert!(!response.text().contains("THEIRS-1"));
}

#[tokio::test]
async fn test_htmx_delete_vehicle() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "delete_test").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Old", "Car", "OLD-1").await;

    let response = env
        .server
        .delete(&format!("/vehicles/htmx/{}", vehicle_id))
        .with_auth(&user.token)
        .await;

    response.assert_status_ok();
    assert_eq!(response.text(), "");
    assert!(!vehicle_exists(&env.pool, vehicle_id).await);
}

#[tokio::test]
async fn test_htmx_delete_vehicle_when_not_owner_returns_not_found() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "delete_owner").await;
    let other = common::create_test_user(&env, "delete_other").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, owner.id, "Kept", "Car", "KEPT-1").await;

    let response = env
        .server
        .delete(&format!("/vehicles/htmx/{}", vehicle_id))
        .with_auth(&other.token)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
    assert!(vehicle_exists(&env.pool, vehicle_id).await);
}

#[tokio::test]
async fn test_htmx_delete_vehicle_when_missing_returns_not_found() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "delete_missing").await;

    let response = env
        .server
        .delete("/vehicles/htmx/999999")
        .with_auth(&user.token)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}
