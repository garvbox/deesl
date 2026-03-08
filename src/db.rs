use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
};
use deadpool_diesel::postgres::{Object, Pool};

use crate::handlers::internal_error;

/// Custom extractor for database connections.
/// This simplifies handlers by removing the need to manually call `pool.get().await`.
pub struct DbConn(pub Object);

impl<S> FromRequestParts<S> for DbConn
where
    Pool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = Pool::from_ref(state);
        let conn = pool.get().await.map_err(internal_error)?;
        Ok(DbConn(conn))
    }
}
