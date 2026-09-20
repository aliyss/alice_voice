//! Failure of the built in GLiNER resolver.

use thiserror::Error;

/// Failure of the built in GLiNER resolver.
#[derive(Debug, Error)]
pub enum GlinerError {
    /// The model is not on disk.
    #[error("the GLiNER model `{0}` is not installed")]
    NotInstalled(String),
    /// The identifier names no model the daemon knows.
    #[error("`{id}` is not a GLiNER model this daemon knows")]
    UnknownModel {
        /// Identifier the caller sent.
        id: String,
    },
    /// Another download runs.
    #[error("a GLiNER download already runs")]
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
    #[error("GLiNER cannot run on {device}: {reason}")]
    Device {
        /// Device the caller asked for.
        device: String,
        /// Why the device is not usable.
        reason: String,
    },
    /// The model did not load.
    #[error("the GLiNER model did not load: {0}")]
    Load(String),
    /// The model did not answer.
    #[error("the GLiNER model did not answer: {0}")]
    Inference(String),
}
