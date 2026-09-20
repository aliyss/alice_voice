//! Failure of an intent write action.

use thiserror::Error;
use uuid::Uuid;

/// Failure of the create, update, or delete action of an intent.
#[derive(Debug, Error)]
pub enum SaveIntentError {
    /// The caller sent a value that the intent rules reject.
    #[error("{reason}")]
    Invalid {
        /// Message for the user.
        reason: String,
    },
    /// Another intent already uses the name.
    #[error("an intent named {name} already exists")]
    NameTaken {
        /// Name the caller sent.
        name: String,
    },
    /// The intent does not exist.
    #[error("intent {id} does not exist")]
    NotFound {
        /// Identifier the caller sent.
        id: Uuid,
    },
    /// The database rejected the write.
    #[error("database failed: {0}")]
    Database(#[from] sea_orm::DbErr),
}

/// Build an invalid input failure with a message for the user.
pub fn invalid(reason: impl Into<String>) -> SaveIntentError {
    SaveIntentError::Invalid {
        reason: reason.into(),
    }
}
