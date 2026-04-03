use figment::Figment;
use figment::providers::Env;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub oauth: OauthConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub dev: DevConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: usize,
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OauthConfig {
    #[serde(default)]
    pub google_client_id: String,
    #[serde(default)]
    pub google_client_secret: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiration")]
    pub jwt_expiration_hours: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DevConfig {
    #[serde(default)]
    pub dev_auth_email: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> usize {
    8000
}

fn default_base_url() -> String {
    "http://localhost:8000".to_string()
}

fn default_jwt_expiration() -> i64 {
    24 * 7
}

impl AppConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config: Self = Figment::new().merge(Env::raw().split("_")).extract()?;

        if config.server.host.is_empty() {
            config.server.host = default_host();
            config.server.port = default_port();
            config.server.base_url = default_base_url();
        }

        if config.oauth.google_client_id.is_empty() {
            config.oauth.google_client_id = std::env::var("GOOGLE_CLIENT_ID")?;
        }
        if config.oauth.google_client_secret.is_empty() {
            config.oauth.google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET")?;
        }
        if config.auth.jwt_secret.is_empty() {
            config.auth.jwt_secret = std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".to_string());
        }
        if config.auth.jwt_expiration_hours == 0 {
            config.auth.jwt_expiration_hours = default_jwt_expiration();
        }

        Ok(config)
    }
}
