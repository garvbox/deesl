use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

pub const JWT_SECRET_KEY: &str = "JWT_SECRET";
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
}

impl AuthConfig {
    pub fn new() -> Self {
        Self {
            secret: std::env::var(JWT_SECRET_KEY)
                .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
        }
    }

    pub fn create_token(
        &self,
        user_id: i32,
        email: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(JWT_EXPIRATION_HOURS))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_secret(secret: &str) -> AuthConfig {
        AuthConfig {
            secret: secret.to_string(),
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
    fn test_new_uses_fallback_secret_when_env_var_absent() {
        // Safety: single-threaded test context; no other threads reading this var
        unsafe { std::env::remove_var(JWT_SECRET_KEY) };
        let config = AuthConfig::new();
        assert_eq!(config.secret, "dev-secret-change-in-production");
    }

    #[test]
    fn test_new_uses_env_var_when_set() {
        // Safety: single-threaded test context; no other threads reading this var
        unsafe { std::env::set_var(JWT_SECRET_KEY, "my-prod-secret") };
        let config = AuthConfig::new();
        assert_eq!(config.secret, "my-prod-secret");
        unsafe { std::env::remove_var(JWT_SECRET_KEY) };
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
