use axum::extract::multipart::MultipartError;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use deadpool_diesel::{InteractError as DeadpoolInteractError, PoolError};
use diesel::result::Error as DieselError;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Internal(String),
    Database(DatabaseError),
}

#[derive(Debug)]
pub enum DatabaseError {
    NotFound,
    UniqueViolation(String),
    ForeignKeyViolation(String),
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "{}", msg),
            AppError::BadRequest(msg) => write!(f, "{}", msg),
            AppError::Unauthorized(msg) => write!(f, "{}", msg),
            AppError::Forbidden(msg) => write!(f, "{}", msg),
            AppError::Internal(msg) => write!(f, "{}", msg),
            AppError::Database(err) => write!(f, "{}", err),
        }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::NotFound => write!(f, "Record not found"),
            DatabaseError::UniqueViolation(msg) => write!(f, "{}", msg),
            DatabaseError::ForeignKeyViolation(msg) => write!(f, "{}", msg),
            DatabaseError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Database(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        tracing::error!("Error: {:?}", self);

        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head><title>{}</title></head>
<body>
<h1>{}</h1>
<p>{}</p>
</body>
</html>"#,
            status.canonical_reason().unwrap_or("Error"),
            status.canonical_reason().unwrap_or("Error"),
            message
        );

        (status, Html(body)).into_response()
    }
}

impl From<PoolError> for AppError {
    fn from(err: PoolError) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<DeadpoolInteractError> for AppError {
    fn from(err: DeadpoolInteractError) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<DieselError> for AppError {
    fn from(err: DieselError) -> Self {
        match err {
            DieselError::NotFound => AppError::Database(DatabaseError::NotFound),
            DieselError::DatabaseError(kind, info) => {
                let msg = info.message().to_string();
                match kind {
                    diesel::result::DatabaseErrorKind::UniqueViolation => {
                        AppError::Database(DatabaseError::UniqueViolation(msg))
                    }
                    diesel::result::DatabaseErrorKind::ForeignKeyViolation => {
                        AppError::Database(DatabaseError::ForeignKeyViolation(msg))
                    }
                    _ => AppError::Database(DatabaseError::Other(msg)),
                }
            }
            _ => AppError::Internal(err.to_string()),
        }
    }
}

impl From<askama::Error> for AppError {
    fn from(err: askama::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<MultipartError> for AppError {
    fn from(err: MultipartError) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<axum_csrf::CsrfError> for AppError {
    fn from(err: axum_csrf::CsrfError) -> Self {
        AppError::Forbidden(err.to_string())
    }
}
