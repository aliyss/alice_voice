//! Failure of one resolution.

use thiserror::Error;

/// Failure of the resolve action.
#[derive(Debug, Error)]
pub enum ResolveError {
    /// No intent is configured, so there is nothing to choose from.
    #[error("no intent is configured")]
    NoIntents,
    /// The decision carries more options than the interface supports.
    #[error("the resolver takes at most {max} options and got {count}")]
    TooManyOptions {
        /// Largest option count of one decision.
        max: usize,
        /// Option count the caller sent.
        count: usize,
    },
    /// The daemon could not reach the model server.
    #[error("the resolver did not answer: {0}")]
    Unreachable(String),
    /// The model server answered with a failure status.
    #[error("the resolver answered with status {status}: {body}")]
    Rejected {
        /// HTTP status of the reply.
        status: u16,
        /// Start of the reply body.
        body: String,
    },
    /// The model answered without a usable choice.
    #[error("the resolver answered without a choice")]
    NoChoice,
    /// The embedding server did not answer with one vector per text.
    #[error("the embedding server answered with no usable vector: {reason}")]
    Embeddings {
        /// What the reply of the server was missing.
        reason: String,
    },
    /// The daemon could not set the resolver up.
    #[error("the resolver is not ready: {0}")]
    Configuration(String),
    /// The built in GLiNER resolver cannot read the message.
    #[error("the built in GLiNER resolver is not ready: {reason}")]
    GlinerNotReady {
        /// What keeps the resolver from reading the message.
        reason: String,
    },
    /// The database rejected a read.
    #[error("database failed: {0}")]
    Database(#[from] sea_orm::DbErr),
}
