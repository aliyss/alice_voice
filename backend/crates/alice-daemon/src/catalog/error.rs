//! Failure of the command catalog.

use thiserror::Error;

/// Failure of the command catalog.
#[derive(Debug, Error)]
pub enum CatalogError {
    /// The daemon could not run one tool the catalog needs.
    #[error("the catalog could not run {tool}: {reason}")]
    Unavailable {
        /// The tool the daemon tried to run.
        tool: String,
        /// What the tool answered, or why it did not run.
        reason: String,
    },
}
