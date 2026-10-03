use axum::http::StatusCode;

mod common;

use common::AuthenticatedRequest;

#[tokio::test]
async fn test_root_redirects_to_dashboard() {
    let env = common::create_test_env().await;
    let response = env.server.get("/").await;

    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("location"), "/dashboard");
}

#[tokio::test]
async fn test_health_returns_ok() {
    let env = common::create_test_env().await;
    let response = env.server.get("/health").await;

    response.assert_status_ok();
    assert_eq!(response.json::<serde_json::Value>()["status"], "ok");
}

#[tokio::test]
async fn test_api_version_returns_version() {
    let env = common::create_test_env().await;
    let response = env.server.get("/api/version").await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<serde_json::Value>()["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[tokio::test]
async fn test_login_page_renders() {
    let env = common::create_test_env().await;
    let response = env.server.get("/login").await;

    response.assert_status_ok();
    assert_eq!(response.content_type(), "text/html; charset=utf-8");
}

#[tokio::test]
async fn test_dashboard_requires_auth_and_redirects() {
    let env = common::create_test_env().await;

    let response = env.server.get("/dashboard").await;

    common::assert_login_redirect(&response);
}

#[tokio::test]
async fn test_dashboard_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "dashboard_test").await;

    let response = env.server.get("/dashboard").with_auth(&user.token).await;

    response.assert_status_ok();
    assert_eq!(response.content_type(), "text/html; charset=utf-8");
    assert!(response.text().contains("Dashboard"));
    assert!(response.text().contains("Your Vehicles"));
}
