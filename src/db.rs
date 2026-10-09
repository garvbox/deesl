use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use deadpool_diesel::postgres::{Object, Pool};
use diesel::RunQueryDsl;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::error::AppError;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

const MIGRATION_LOCK_KEY: i64 = 7_271_001;

pub async fn run_migrations(pool: &Pool) -> Result<(), AppError> {
    let conn = pool.get().await?;

    conn.interact(|conn| {
        // Serialize concurrent migration attempts (e.g. parallel tests sharing a
        // database) so they don't race on the migrations table.
        diesel::sql_query(format!("SELECT pg_advisory_lock({MIGRATION_LOCK_KEY})"))
            .execute(conn)
            .map_err(|err| {
                AppError::Internal(format!("Failed to acquire migration lock: {err}"))
            })?;

        let result = conn
            .run_pending_migrations(MIGRATIONS)
            .map(|_| ())
            .map_err(|err| AppError::Internal(format!("Failed to run migrations: {err}")));

        let _ = diesel::sql_query(format!("SELECT pg_advisory_unlock({MIGRATION_LOCK_KEY})"))
            .execute(conn);

        result
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
