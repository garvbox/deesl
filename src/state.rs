use axum::extract::FromRef;
use deadpool_diesel::postgres::Pool;

use crate::oauth_handlers::OAuthConfig;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub oauth: OAuthConfig,
}

impl FromRef<AppState> for Pool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}
