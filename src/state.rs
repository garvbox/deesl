use axum::extract::FromRef;
use axum_csrf::CsrfConfig;
use deadpool_diesel::postgres::Pool;

use crate::auth::AuthConfig;
use crate::oauth_handlers::OAuthConfig;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub oauth: OAuthConfig,
    pub auth: AuthConfig,
    pub csrf: CsrfConfig,
}

impl FromRef<AppState> for Pool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for AuthConfig {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}

impl FromRef<AppState> for CsrfConfig {
    fn from_ref(state: &AppState) -> Self {
        state.csrf.clone()
    }
}
