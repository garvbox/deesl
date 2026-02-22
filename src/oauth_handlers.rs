use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect},
    routing::get,
};
use diesel::prelude::*;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl, basic::BasicClient, reqwest::async_http_client,
};
use serde::Deserialize;
use std::env;

use crate::auth::AuthConfig;
use crate::handlers::internal_error;
use crate::models::User;
use crate::schema::users;
use crate::state::AppState;

const CSRF_COOKIE: &str = "oauth_csrf";

#[derive(Clone)]
pub struct OAuthConfig {
    pub client: BasicClient,
}

impl OAuthConfig {
    pub fn new() -> Self {
        let client_id = env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID must be set");
        let client_secret =
            env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET must be set");
        let redirect_url = env::var("OAUTH_REDIRECT_URL")
            .unwrap_or_else(|_| "http://localhost:8000/api/auth/google/callback".to_string());

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
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/google", get(google_login))
        .route("/api/auth/google/callback", get(google_callback))
}

pub async fn google_login(State(state): State<AppState>) -> impl IntoResponse {
    let (auth_url, csrf_token) = state
        .oauth
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .url();

    let cookie = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600",
        CSRF_COOKIE,
        csrf_token.secret()
    );

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

    let clear_cookie = format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        CSRF_COOKIE
    );
    let redirect_url = format!(
        "/?token={}&user_id={}&email={}",
        jwt,
        user.id,
        urlencoding::encode(&user.email)
    );

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::SET_COOKIE, clear_cookie.parse().unwrap());

    Ok((resp_headers, Redirect::to(&redirect_url)))
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
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
