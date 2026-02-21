use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

const JWT_SECRET_KEY: &str = "JWT_SECRET";
const JWT_EXPIRATION_HOURS: i64 = 24 * 7;

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

    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
    }
}
