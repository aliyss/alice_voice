//! Download of one model file.
//!
//! A model is a set of files in one directory. The daemon downloads a file
//! into a part file and renames it when it is complete, so an interrupted
//! download never looks like an installed model.
//!
//! Both stores of the daemon use this module: the store of the built in
//! GLiNER models and the store of the built in models of the router. The
//! files differ and the progress reporting is the same, so the fetch lives
//! in one place.

use std::path::Path;

use futures::StreamExt;
use tokio::io::AsyncWriteExt;

/// Suffix of a file that is still downloading.
pub const PART_SUFFIX: &str = "part";

/// Failure of one download.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// The download did not finish.
    #[error("{0}")]
    Failed(String),
}

/// Download one file into its place.
///
/// The caller names the address and the target, and reads the progress
/// through `on_chunk`. The callback receives the length of the chunk that
/// arrived and the length the server announced, or none when the server
/// announced nothing.
pub async fn fetch_file(
    url: &str,
    target: &Path,
    mut on_chunk: impl FnMut(u64, Option<u64>),
) -> Result<(), DownloadError> {
    let part = target.with_extension(PART_SUFFIX);
    if let Some(parent) = part.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(failed)?;
    }

    let response = reqwest::get(url).await.map_err(failed)?;
    if !response.status().is_success() {
        return Err(DownloadError::Failed(format!(
            "the server answered with status {}",
            response.status()
        )));
    }
    let announced = response.content_length();

    let mut handle = tokio::fs::File::create(&part).await.map_err(failed)?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(failed)?;
        handle.write_all(&chunk).await.map_err(failed)?;
        on_chunk(chunk.len() as u64, announced);
    }
    handle.flush().await.map_err(failed)?;
    drop(handle);

    tokio::fs::rename(&part, target).await.map_err(failed)?;
    Ok(())
}

/// Build a download failure out of an error of the operating system or
/// the network.
fn failed(err: impl std::fmt::Display) -> DownloadError {
    DownloadError::Failed(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_file_is_told_apart_from_a_finished_file() {
        let target = Path::new("/models/bge/tokenizer.json");
        assert_eq!(
            target.with_extension(PART_SUFFIX),
            Path::new("/models/bge/tokenizer.part")
        );
    }

    #[tokio::test]
    async fn an_address_that_answers_nothing_fails_with_a_reason() {
        let target = std::env::temp_dir().join("alice-download-test/one.bin");
        let err = fetch_file("http://127.0.0.1:1/nothing", &target, |_, _| {})
            .await
            .expect_err("nothing answers on that port");

        assert!(!err.to_string().is_empty());
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("alice-download-test"));
    }
}
