//! Centralised error type so commands can return `Result<T, AppError>`.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("migration: {0}")]
    Migration(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("permission denied: {0}")]
    Permission(String),

    #[error("upstream: {0}")]
    Upstream(String),

    #[error("cancelled")]
    Cancelled,

    #[error("internal: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Helper to convert a `reqwest::Error` into an `AppError::Upstream` with a
/// human-readable message. We surface the underlying error verbatim so the
/// UI can show the real cause (DNS failure, connection refused, timeout, ...).
pub fn upstream<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Upstream(e.to_string())
}
