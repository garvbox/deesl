use axum::{
    extract::{FromRef, FromRequestParts},
    http::{HeaderMap, Method, StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
    middleware::Next,
    body::{Body, Bytes},
};
use axum_csrf::CsrfToken;
use deadpool_diesel::postgres::Pool;
use diesel::prelude::*;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::oauth_handlers::extract_cookie;
use crate::schema::users;

// ... (AuthConfig and Claims)

pub async fn csrf_middleware(
    token: CsrfToken,
    req: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    if req.method() == Method::GET
        || req.method() == Method::HEAD
        || req.method() == Method::OPTIONS
    {
        return Ok(next.run(req).await);
    }

    tracing::debug!("CSRF Check for method: {}", req.method());
    tracing::debug!("Headers: {:?}", req.headers());
    if let Some(cookies) = req.headers().get(axum::http::header::COOKIE) {
        tracing::debug!("Cookies: {:?}", cookies);
    } else {
        tracing::debug!("No cookies found");
    }

    // Check header first (HTMX)
    if let Some(h) = req.headers().get("X-CSRF-Token").and_then(|v| v.to_str().ok()) {
        tracing::debug!("Checking token from header: {}", h);
        if token.verify(h).is_ok() {
            return Ok(next.run(req).await);
        }
        tracing::debug!("Token verification failed for header");
    }

    // For multipart forms (like file upload), we don't want to buffer the whole thing if it's huge.
    // But we need the token.
    // Usually HTMX sends multipart if it has files.
    // If it's a standard form (application/x-www-form-urlencoded), we buffer.
    
    let is_form = req.headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/x-www-form-urlencoded"))
        .unwrap_or(false);

    if is_form {
        let (parts, body) = req.into_parts();
        let bytes = axum::body::to_bytes(body, 1024 * 1024) // 1MB limit
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read body: {}", e)))?;

        // Check for authenticity_token in form data
        let params: Vec<(String, String)> = serde_urlencoded::from_bytes(&bytes)
            .map_err(|_| AppError::BadRequest("Invalid form data".to_string()))?;

        let form_token = params.iter().find(|(k, _)| k == "authenticity_token").map(|(_, v)| v);

        if let Some(t) = form_token {
            if token.verify(t).is_ok() {
                let req = axum::extract::Request::from_parts(parts, Body::from(bytes));
                return Ok(next.run(req).await);
            }
        }
    }

    Err(AppError::Forbidden("Invalid CSRF token".to_string()))
}

pub const JWT_SECRET_KEY: &str = "JWT_SECRET";
pub const DEV_AUTH_EMAIL_KEY: &str = "DEV_AUTH_EMAIL";
pub const JWT_EXPIRATION_HOURS: i64 = 24 * 7;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub user_id: i32,
    pub exp: i64,
}

#[derive(Clone)]
pub struct AuthConfig {
    pub secret: String,
    pub expiration_hours: i64,
}

impl AuthConfig {
    pub fn new(secret: &str, expiration_hours: i64) -> Self {
        Self {
            secret: secret.to_string(),
            expiration_hours,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            secret: "dev-secret-change-in-production".to_string(),
            expiration_hours: 24 * 7,
        }
    }
}

impl AuthConfig {
    pub fn create_token(
        &self,
        user_id: i32,
        email: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(self.expiration_hours))
            .unwrap()
            .timestamp();

        let claims = Claims {
            sub: email.to_string(),
            user_id,
            exp: expiration,
        };

        let header = Header::default();
        encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    #[allow(dead_code)]
    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i32,
    pub email: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    AuthConfig: FromRef<S>,
    Pool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_config = AuthConfig::from_ref(state);
        let pool = Pool::from_ref(state);
        extract_auth_user(&parts.headers, &auth_config, &pool).await
    }
}

/// A wrapper for AuthUser that redirects to /login on failure for standard page requests.
pub struct AuthUserRedirect(pub AuthUser);

impl<S> FromRequestParts<S> for AuthUserRedirect
where
    AuthConfig: FromRef<S>,
    Pool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Redirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_config = AuthConfig::from_ref(state);
        let pool = Pool::from_ref(state);
        match extract_auth_user(&parts.headers, &auth_config, &pool).await {
            Ok(user) => Ok(AuthUserRedirect(user)),
            Err(_) => Err(Redirect::to("/login")),
        }
    }
}

pub fn is_dev_auth_bypass_allowed(_headers: &HeaderMap) -> Option<String> {
    #[cfg(feature = "dev")]
    {
        std::env::var(DEV_AUTH_EMAIL_KEY).ok()
    }
    #[cfg(not(feature = "dev"))]
    {
        None
    }
}

pub async fn extract_auth_user(
    headers: &HeaderMap,
    auth_config: &AuthConfig,
    pool: &Pool,
) -> Result<AuthUser, AppError> {
    if let Some(email) = is_dev_auth_bypass_allowed(headers) {
        return Ok(AuthUser { user_id: 1, email });
    }

    let token = extract_cookie(headers, "auth_token")
        .ok_or_else(|| AppError::Unauthorized("Missing auth token".to_string()))?;

    let claims = auth_config
        .validate_token(&token)
        .map_err(|_| AppError::Unauthorized("Invalid token".to_string()))?;

    let user_id = claims.user_id;
    let conn = pool.get().await?;
    let user_exists = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(user_id))
                .select(users::id)
                .first::<i32>(conn)
                .optional()
        })
        .await??
        .is_some();

    if !user_exists {
        return Err(AppError::Unauthorized("User no longer exists".to_string()));
    }

    Ok(AuthUser {
        user_id: claims.user_id,
        email: claims.sub,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_secret(secret: &str) -> AuthConfig {
        AuthConfig {
            secret: secret.to_string(),
            expiration_hours: 24 * 7,
        }
    }

    #[test]
    fn test_create_token_produces_valid_jwt_structure() {
        let config = config_with_secret("test-secret");
        let token = config.create_token(42, "user@example.com").unwrap();
        // A JWT has three base64url segments separated by dots
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn test_validate_token_round_trips_claims() {
        let config = config_with_secret("test-secret");
        let token = config.create_token(42, "user@example.com").unwrap();
        let claims = config.validate_token(&token).unwrap();
        assert_eq!(claims.user_id, 42);
        assert_eq!(claims.sub, "user@example.com");
    }

    #[test]
    fn test_validate_token_rejects_wrong_secret() {
        let creator = config_with_secret("correct-secret");
        let validator = config_with_secret("wrong-secret");
        let token = creator.create_token(1, "a@b.com").unwrap();
        assert!(validator.validate_token(&token).is_err());
    }

    #[test]
    fn test_validate_token_rejects_tampered_payload() {
        let config = config_with_secret("test-secret");
        let token = config.create_token(1, "a@b.com").unwrap();
        // Flip a character in the payload (middle) segment
        let mut parts: Vec<&str> = token.split('.').collect();
        let mut payload = parts[1].to_string();
        let tampered = if payload.ends_with('A') {
            payload.push('B');
            payload
        } else {
            payload.push('A');
            payload
        };
        parts[1] = &tampered;
        let tampered_token = parts.join(".");
        assert!(config.validate_token(&tampered_token).is_err());
    }

    #[test]
    fn test_validate_token_rejects_expired_token() {
        let config = config_with_secret("test-secret");
        // Manually build a token with an exp in the past
        let claims = Claims {
            sub: "a@b.com".to_string(),
            user_id: 1,
            exp: chrono::Utc::now().timestamp() - 3600,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )
        .unwrap();
        assert!(config.validate_token(&token).is_err());
    }

    #[test]
    fn test_token_expiry_is_approximately_seven_days_from_now() {
        let config = config_with_secret("test-secret");
        let before = chrono::Utc::now().timestamp();
        let token = config.create_token(1, "a@b.com").unwrap();
        let after = chrono::Utc::now().timestamp();
        let claims = config.validate_token(&token).unwrap();
        let expected_min = before + JWT_EXPIRATION_HOURS * 3600;
        let expected_max = after + JWT_EXPIRATION_HOURS * 3600;
        assert!(claims.exp >= expected_min && claims.exp <= expected_max);
    }
}
