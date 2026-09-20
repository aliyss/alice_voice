//! Core error type of the backend.
//! This module defines `CoreError` and `Result`.

use thiserror::Error;

/// Core error of the backend.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Database operation failed.
    #[error("database failed: {reason}")]
    DatabaseFailed { reason: String },

    /// Validation failed for the request body.
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },

    /// Configuration is invalid.
    #[error("config invalid: {reason}")]
    ConfigInvalid { reason: String },

    /// Internal error that does not fit another variant.
    #[error("internal error: {reason}")]
    Internal { reason: String },
}

/// Result that uses `CoreError`.
pub type Result<T> = std::result::Result<T, CoreError>;
