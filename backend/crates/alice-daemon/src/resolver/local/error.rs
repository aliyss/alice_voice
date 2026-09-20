//! Failure of the built in models of the router.

use thiserror::Error;

/// Failure of the built in models of the router.
#[derive(Debug, Error)]
pub enum LocalError {
    /// The model is not on disk.
    #[error("the model `{0}` is not installed")]
    NotInstalled(String),
    /// The identifier names no model the daemon knows.
    #[error("`{id}` is not a built in model this daemon knows")]
    UnknownModel {
        /// Identifier the caller sent.
        id: String,
    },
    /// The identifier names a model of another role.
    #[error("`{id}` is not a {role} model")]
    WrongRole {
        /// Identifier the caller sent.
        id: String,
        /// Role the caller asked for.
        role: &'static str,
    },
    /// Another download runs.
    #[error("a download of a built in model already runs")]
    DownloadRunning,
    /// The download failed.
    #[error("the download of `{id}` failed: {reason}")]
    Download {
        /// Identifier of the model.
        id: String,
        /// What went wrong.
        reason: String,
    },
    /// The build or the machine cannot run on the requested device.
    #[error("the model cannot run on {device}: {reason}")]
    Device {
        /// Device the caller asked for.
        device: String,
        /// Why the device is not usable.
        reason: String,
    },
    /// The model did not load.
    #[error("the model did not load: {0}")]
    Load(String),
    /// The text could not be turned into tokens.
    #[error("the text could not be tokenized: {0}")]
    Tokenize(String),
    /// The model did not answer.
    #[error("the model did not answer: {0}")]
    Inference(String),
}
