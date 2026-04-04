use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: usize,
    pub base_url: String,
    pub database_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub dev_auth_email: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(AppConfig {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(8000),
            base_url: std::env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            database_url: std::env::var("DATABASE_URL")?,
            google_client_id: std::env::var("GOOGLE_CLIENT_ID")?,
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET")?,
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
            jwt_expiration_hours: std::env::var("JWT_EXPIRATION_HOURS")
                .unwrap_or_default()
                .parse()
                .unwrap_or(24 * 7),
            dev_auth_email: std::env::var("DEV_AUTH_EMAIL").ok(),
        })
    }
}
