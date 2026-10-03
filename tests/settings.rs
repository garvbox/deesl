mod common;

use common::AuthenticatedRequest;

#[tokio::test]
async fn test_settings_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "settings_page").await;

    let response = env.server.get("/settings").with_auth(&user.token).await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("Settings"));
    assert!(body.contains("Preferences"));
}

#[tokio::test]
async fn test_settings_can_update_currency() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "settings_test").await;

    let form = [
        ("currency", "USD"),
        ("distance_unit", "mi"),
        ("volume_unit", "gal"),
    ];

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
async fn test_update_settings_when_invalid_currency_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "settings_bad_currency").await;

    let form = [
        ("currency", "XYZ"),
        ("distance_unit", "km"),
        ("volume_unit", "L"),
    ];

    let response = env
        .server
        .patch("/settings")
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_settings_when_invalid_distance_unit_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "settings_bad_distance").await;

    let form = [
        ("currency", "EUR"),
        ("distance_unit", "furlongs"),
        ("volume_unit", "L"),
    ];

    let response = env
        .server
        .patch("/settings")
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_settings_when_invalid_volume_unit_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "settings_bad_volume").await;

    let form = [
        ("currency", "EUR"),
        ("distance_unit", "km"),
        ("volume_unit", "barrels"),
    ];

    let response = env
        .server
        .patch("/settings")
        .with_auth(&user.token)
        .form(&form)
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}
