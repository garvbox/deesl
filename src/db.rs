use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use deadpool_diesel::postgres::{Object, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::error::AppError;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub async fn run_migrations(pool: &Pool) -> Result<(), AppError> {
    let conn = pool.get().await?;

    conn.interact(|conn| {
        conn.run_pending_migrations(MIGRATIONS)
            .map(|_| ())
            .map_err(|err| AppError::Internal(format!("Failed to run migrations: {err}")))
    })
    .await?
}

/// Custom extractor for database connections.
/// This simplifies handlers by removing the need to manually call `pool.get().await`.
pub struct DbConn(pub Object);

impl<S> FromRequestParts<S> for DbConn
where
    Pool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = Pool::from_ref(state);
        let conn = pool.get().await?;
        Ok(DbConn(conn))
    }
}
