use axum::http::StatusCode;
use diesel::prelude::*;

mod common;

use common::AuthenticatedRequest;

#[tokio::test]
async fn test_logout_redirects_and_clears_cookie() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "logout_test").await;

    let response = env.server.get("/auth/logout").with_auth(&user.token).await;

    response.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(response.header("location"), "/login");

    let cookie = response.header("set-cookie").to_str().unwrap().to_string();
    assert!(cookie.contains("auth_token=;"));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn test_api_auth_me_returns_user_when_authenticated() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "me_test").await;

    let response = env.server.get("/api/auth/me").with_auth(&user.token).await;

    response.assert_status_ok();
    let body = response.json::<serde_json::Value>();
    assert_eq!(body["user_id"], user.id);
    assert!(body["email"].as_str().unwrap().starts_with("me_test_"));
}

#[tokio::test]
async fn test_api_auth_me_returns_unauthorized_when_unauthenticated() {
    let env = common::create_test_env().await;
    let response = env.server.get("/api/auth/me").await;

    common::assert_unauthorized(&response);
}

#[tokio::test]
async fn test_auth_user_endpoints_return_unauthorized_when_unauthenticated() {
    let env = common::create_test_env().await;

    let endpoints = [
        ("/vehicles/htmx/list", "get"),
        ("/vehicles/htmx/1", "delete"),
        ("/fuel-entries/htmx/recent", "get"),
        ("/fuel-entries/htmx/1", "delete"),
        ("/stations/htmx/search?q=abc", "get"),
    ];

    for (path, method) in endpoints {
        let response = match method {
            "get" => env.server.get(path).await,
            "delete" => env.server.delete(path).await,
            _ => unreachable!(),
        };
        common::assert_unauthorized(&response);
    }
}

#[tokio::test]
async fn test_auth_user_redirect_pages_redirect_when_unauthenticated() {
    let env = common::create_test_env().await;

    let pages = [
        "/dashboard",
        "/settings",
        "/vehicles",
        "/vehicles/new",
        "/fuel-entries",
        "/fuel-entries/new",
        "/stations",
        "/stats",
        "/import",
    ];

    for page in pages {
        let response = env.server.get(page).await;
        common::assert_login_redirect(&response);
    }
}

#[tokio::test]
async fn test_invalid_token_returns_unauthorized() {
    let env = common::create_test_env().await;
    let response = env
        .server
        .get("/api/auth/me")
        .with_auth("not-a-valid-jwt")
        .await;

    common::assert_unauthorized(&response);
}

#[tokio::test]
async fn test_expired_token_returns_unauthorized() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "expired_test").await;

    let claims = deesl::auth::Claims {
        sub: format!("expired_{}@test.com", uuid::Uuid::new_v4()),
        user_id: user.id,
        exp: chrono::Utc::now().timestamp() - 3600,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap();

    let response = env.server.get("/api/auth/me").with_auth(&token).await;

    common::assert_unauthorized(&response);
}

#[tokio::test]
async fn test_token_for_deleted_user_returns_unauthorized() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "deleted_test").await;

    let conn = env.pool.get().await.unwrap();
    let user_id = user.id;
    conn.interact(move |conn| {
        diesel::delete(deesl::schema::users::table.filter(deesl::schema::users::id.eq(user_id)))
            .execute(conn)
    })
    .await
    .unwrap()
    .unwrap();

    let response = env.server.get("/api/auth/me").with_auth(&user.token).await;

    common::assert_unauthorized(&response);
}

#[tokio::test]
async fn test_google_login_redirects_to_google_and_sets_csrf_cookie() {
    let env = common::create_test_env().await;
    let response = env.server.get("/auth/google").await;

    response.assert_status(StatusCode::FOUND);
    assert!(
        response
            .header("location")
            .to_str()
            .unwrap()
            .starts_with("https://accounts.google.com")
    );
    assert!(
        response
            .header("set-cookie")
            .to_str()
            .unwrap()
            .contains("oauth_csrf=")
    );
}

#[tokio::test]
async fn test_google_callback_returns_bad_request_when_csrf_cookie_missing() {
    let env = common::create_test_env().await;
    let response = env
        .server
        .get("/auth/google/callback?code=abc&state=xyz")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_google_callback_returns_bad_request_when_csrf_state_mismatch() {
    let env = common::create_test_env().await;
    let response = env
        .server
        .get("/auth/google/callback?code=abc&state=xyz")
        .add_header("Cookie", "oauth_csrf=different")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}
