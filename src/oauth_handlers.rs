use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use diesel::prelude::*;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl, basic::BasicClient, reqwest::async_http_client,
};
use serde::Deserialize;
use std::env;

use crate::auth::{AuthConfig, extract_auth_user};
use crate::handlers::internal_error;
use crate::models::User;
use crate::schema::users;
use crate::state::AppState;

const CSRF_COOKIE: &str = "oauth_csrf";

fn is_development() -> bool {
    env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()) == "development"
}

fn build_cookie(name: &str, value: &str, max_age: i64) -> String {
    let secure_flag = if is_development() { "" } else { "; Secure" };
    format!(
        "{}={}; HttpOnly; SameSite=Lax{}; Path=/; Max-Age={}",
        name, value, secure_flag, max_age
    )
}

fn build_clear_cookie(name: &str) -> String {
    let secure_flag = if is_development() { "" } else { "; Secure" };
    format!(
        "{}=; HttpOnly; SameSite=Lax{}; Path=/; Max-Age=0",
        name, secure_flag
    )
}

#[derive(Clone)]
pub struct OAuthConfig {
    pub client: BasicClient,
}

impl OAuthConfig {
    pub fn new(base_url: &str) -> Self {
        let client_id = env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID must be set");
        let client_secret =
            env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET must be set");
        let redirect_url = format!("{}/api/auth/google/callback", base_url);

        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .expect("Invalid auth URL"),
            Some(
                TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                    .expect("Invalid token URL"),
            ),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).expect("Invalid redirect URL"));

        Self { client }
    }

    pub fn test_config() -> Self {
        // Create a dummy client for testing (won't be used for actual OAuth flow)
        let client = BasicClient::new(
            ClientId::new("test-client-id".to_string()),
            Some(ClientSecret::new("test-client-secret".to_string())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .expect("Invalid auth URL"),
            Some(
                TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                    .expect("Invalid token URL"),
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new("http://localhost:8000/api/auth/google/callback".to_string())
                .expect("Invalid redirect URL"),
        );

        Self { client }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/google", get(google_login))
        .route("/api/auth/google/callback", get(google_callback))
        .route("/api/auth/me", get(get_current_user))
        .route("/api/auth/logout", post(logout))
}

pub async fn google_login(State(state): State<AppState>) -> impl IntoResponse {
    let (auth_url, csrf_token) = state
        .oauth
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .url();

    let cookie = build_cookie(CSRF_COOKIE, csrf_token.secret(), 600);

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
    headers.insert(header::LOCATION, auth_url.to_string().parse().unwrap());

    (StatusCode::FOUND, headers)
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: String,
}

pub async fn google_callback(
    State(state): State<AppState>,
    req_headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<CallbackParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let csrf_cookie = extract_cookie(&req_headers, CSRF_COOKIE)
        .ok_or((StatusCode::BAD_REQUEST, "Missing CSRF cookie".to_string()))?;

    if csrf_cookie != params.state {
        return Err((StatusCode::BAD_REQUEST, "CSRF state mismatch".to_string()));
    }

    let token_result = state
        .oauth
        .client
        .exchange_code(AuthorizationCode::new(params.code))
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Token exchange failed: {e}"),
            )
        })?;

    let user_info: GoogleUserInfo = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token_result.access_token().secret())
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch user info: {e}"),
            )
        })?
        .json()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse user info: {e}"),
            )
        })?;

    let google_id = user_info.sub.clone();
    let email = user_info.email.clone();
    let conn = state.pool.get().await.map_err(internal_error)?;

    let user: User = conn
        .interact(move |conn| {
            let existing = users::table
                .filter(users::google_id.eq(&google_id))
                .first::<User>(conn)
                .optional()?;

            if let Some(u) = existing {
                return Ok(u);
            }

            let by_email = users::table
                .filter(users::email.eq(&email))
                .first::<User>(conn)
                .optional()?;

            if let Some(u) = by_email {
                return diesel::update(users::table.filter(users::id.eq(u.id)))
                    .set(users::google_id.eq(&google_id))
                    .returning(User::as_returning())
                    .get_result(conn);
            }

            diesel::insert_into(users::table)
                .values((users::email.eq(&email), users::google_id.eq(&google_id)))
                .returning(User::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {e}"),
            )
        })?;

    let jwt = AuthConfig::new()
        .create_token(user.id, &user.email)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create token: {e}"),
            )
        })?;

    // Clear CSRF cookie and set auth cookie with JWT
    let clear_csrf_cookie = build_clear_cookie(CSRF_COOKIE);
    let auth_cookie = build_cookie("auth_token", &jwt, 604800);

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::SET_COOKIE, clear_csrf_cookie.parse().unwrap());
    resp_headers.insert(header::SET_COOKIE, auth_cookie.parse().unwrap());

    Ok((resp_headers, Redirect::to("/")))
}

#[derive(serde::Serialize)]
pub struct CurrentUserResponse {
    pub user_id: i32,
    pub email: String,
}

pub async fn get_current_user(
    req_headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Use extract_auth_user to support dev auth bypass
    let auth_user = extract_auth_user(&req_headers)?;
    
    Ok(Json(CurrentUserResponse {
        user_id: auth_user.user_id,
        email: auth_user.email,
    }))
}

pub async fn logout() -> impl IntoResponse {
    let clear_cookie = build_clear_cookie("auth_token");
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::SET_COOKIE, clear_cookie.parse().unwrap());
    (StatusCode::OK, resp_headers, "Logged out successfully")
}

pub(crate) fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|part| {
                let part = part.trim();
                part.strip_prefix(&format!("{name}="))
                    .map(|v| v.to_string())
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_cookie(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, value.parse().unwrap());
        h
    }

    #[test]
    fn test_extract_cookie_returns_value_when_present() {
        let headers = headers_with_cookie("oauth_csrf=abc123");
        assert_eq!(
            extract_cookie(&headers, "oauth_csrf"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_extract_cookie_returns_none_when_absent() {
        let headers = headers_with_cookie("other_cookie=xyz");
        assert_eq!(extract_cookie(&headers, "oauth_csrf"), None);
    }

    #[test]
    fn test_extract_cookie_returns_none_when_no_cookie_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_cookie(&headers, "oauth_csrf"), None);
    }

    #[test]
    fn test_extract_cookie_finds_cookie_among_multiple() {
        let headers = headers_with_cookie("session=sess1; oauth_csrf=state42; theme=dark");
        assert_eq!(
            extract_cookie(&headers, "oauth_csrf"),
            Some("state42".to_string())
        );
    }

    #[test]
    fn test_extract_cookie_handles_whitespace_around_pairs() {
        // The key=value pair is trimmed as a whole, so trailing spaces on the pair
        // are stripped, but leading spaces in the value are preserved
        let headers = headers_with_cookie("foo=bar;  oauth_csrf=  spaced  ; baz=qux");
        assert_eq!(
            extract_cookie(&headers, "oauth_csrf"),
            Some("  spaced".to_string())
        );
    }

    #[test]
    fn test_extract_cookie_does_not_match_partial_name() {
        // "csrf" should not match "oauth_csrf"
        let headers = headers_with_cookie("oauth_csrf=secret");
        assert_eq!(extract_cookie(&headers, "csrf"), None);
    }

    #[test]
    fn test_extract_cookie_returns_empty_string_value() {
        let headers = headers_with_cookie("oauth_csrf=");
        assert_eq!(extract_cookie(&headers, "oauth_csrf"), Some(String::new()));
    }
}
