//! Download and install state of the built in GLiNER models.
//!
//! A model is two files in one directory. The daemon downloads each file
//! into a part file and renames it when it is complete, so an interrupted
//! download never looks like an installed model.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use alice_core::dto::{GlinerDownloadDto, GlinerModelDto};

use crate::resolver::download;
use crate::resolver::gliner::catalog::{self, ModelSpec};
use crate::resolver::gliner::error::GlinerError;

/// The download that runs right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadState {
    /// Identifier of the model.
    pub model: String,
    /// Bytes written so far.
    pub received: u64,
    /// Size the catalog or the server announced, or none.
    pub total: Option<u64>,
    /// Whether the download finished.
    pub done: bool,
    /// The error that stopped the download, or none.
    pub error: Option<String>,
}

impl DownloadState {
    /// Build the state of a download that just started.
    pub fn started(spec: &ModelSpec) -> Self {
        Self {
            model: spec.id.to_string(),
            received: 0,
            total: Some(spec.size_bytes()),
            done: false,
            error: None,
        }
    }

    /// The DTO the settings page reads.
    pub fn to_dto(&self) -> GlinerDownloadDto {
        GlinerDownloadDto {
            model: self.model.clone(),
            received_bytes: self.received,
            total_bytes: self.total,
            done: self.done,
            error: self.error.clone(),
        }
    }
}

/// The directory that holds the downloaded models.
#[derive(Debug)]
pub struct GlinerStore {
    root: PathBuf,
    state: Mutex<Option<DownloadState>>,
}

impl GlinerStore {
    /// Create a new store.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            state: Mutex::new(None),
        }
    }

    /// The directory the models live in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory of one model.
    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// Whether every file of the model is on disk.
    pub fn is_installed(&self, spec: &ModelSpec) -> bool {
        spec.files
            .iter()
            .all(|file| self.dir(spec.id).join(file.path).is_file())
    }

    /// Every model of the catalog with its install state.
    pub fn list(&self) -> Vec<GlinerModelDto> {
        catalog::CATALOG
            .iter()
            .map(|spec| GlinerModelDto {
                id: spec.id.to_string(),
                name: spec.name.to_string(),
                note: spec.note.to_string(),
                size_bytes: spec.size_bytes(),
                installed: self.is_installed(spec),
            })
            .collect()
    }

    /// The download that runs right now.
    pub fn download_state(&self) -> Option<DownloadState> {
        self.lock().clone()
    }

    /// Look one model up and check that it is on disk.
    pub fn installed_spec(&self, id: &str) -> Result<&'static ModelSpec, GlinerError> {
        let spec =
            catalog::find(id).ok_or_else(|| GlinerError::UnknownModel { id: id.to_string() })?;
        if !self.is_installed(spec) {
            return Err(GlinerError::NotInstalled(spec.id.to_string()));
        }
        Ok(spec)
    }

    /// Start the download of one model in the background.
    pub fn start(self: &Arc<Self>, id: &str) -> Result<(), GlinerError> {
        let spec =
            catalog::find(id).ok_or_else(|| GlinerError::UnknownModel { id: id.to_string() })?;
        {
            let mut state = self.lock();
            let runs = state
                .as_ref()
                .is_some_and(|current| !current.done && current.error.is_none());
            if runs {
                return Err(GlinerError::DownloadRunning);
            }
            *state = Some(DownloadState::started(spec));
        }

        let store = Arc::clone(self);
        tokio::spawn(async move {
            let outcome = store.fetch(spec).await;
            let mut state = store.lock();
            if let Some(current) = state.as_mut() {
                if current.model != spec.id {
                    return;
                }
                match outcome {
                    Ok(()) => current.done = true,
                    Err(err) => current.error = Some(err.to_string()),
                }
            }
        });
        Ok(())
    }

    /// Download every file of one model.
    pub async fn fetch(self: &Arc<Self>, spec: &'static ModelSpec) -> Result<(), GlinerError> {
        tokio::fs::create_dir_all(self.dir(spec.id))
            .await
            .map_err(|err| download_error(spec.id, err))?;
        for file in spec.files {
            self.fetch_file(spec, file).await?;
        }
        Ok(())
    }

    /// Download one file of one model into its place.
    async fn fetch_file(
        &self,
        spec: &ModelSpec,
        file: &catalog::ModelFile,
    ) -> Result<(), GlinerError> {
        let target = self.dir(spec.id).join(file.path);
        download::fetch_file(&spec.url(file), &target, |bytes, announced| {
            self.advance(spec, bytes, announced);
        })
        .await
        .map_err(|err| download_error(spec.id, err))
    }

    /// Add the bytes of one chunk to the progress of the running download.
    fn advance(&self, spec: &ModelSpec, bytes: u64, announced: Option<u64>) {
        let mut state = self.lock();
        let Some(current) = state.as_mut() else {
            return;
        };
        if current.model != spec.id {
            return;
        }
        current.received += bytes;
        if current.total.is_none() {
            current.total = announced.map(|total| current.received + total);
        }
    }

    /// Remove the files of one model.
    pub fn remove(&self, id: &str) -> Result<(), GlinerError> {
        let spec =
            catalog::find(id).ok_or_else(|| GlinerError::UnknownModel { id: id.to_string() })?;
        let dir = self.dir(spec.id);
        if dir.is_dir() {
            std::fs::remove_dir_all(&dir).map_err(|err| download_error(spec.id, err))?;
        }
        let mut state = self.lock();
        if state
            .as_ref()
            .is_some_and(|current| current.model == spec.id && current.done)
        {
            *state = None;
        }
        Ok(())
    }

    /// Lock the progress of the running download.
    fn lock(&self) -> MutexGuard<'_, Option<DownloadState>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Build a download failure.
fn download_error(id: &str, err: impl std::fmt::Display) -> GlinerError {
    GlinerError::Download {
        id: id.to_string(),
        reason: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::gliner::catalog::SMALL;

    /// Build a store in a directory of its own.
    fn store() -> (GlinerStore, PathBuf) {
        let root = std::env::temp_dir().join(format!("alice-gliner-{}", uuid::Uuid::new_v4()));
        (GlinerStore::new(root.clone()), root)
    }

    #[test]
    fn a_model_without_files_is_not_installed() {
        let (store, root) = store();
        assert!(!store.is_installed(&SMALL));
        let list = store.list();
        assert_eq!(list.len(), 3);
        assert!(list.iter().all(|model| !model.installed));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_model_with_every_file_is_installed() {
        let (store, root) = store();
        for file in SMALL.files {
            let path = store.dir(SMALL.id).join(file.path);
            std::fs::create_dir_all(path.parent().expect("the file has a directory"))
                .expect("the directory is created");
            std::fs::write(&path, b"graph").expect("the file is written");
        }

        assert!(store.is_installed(&SMALL));
        assert!(store
            .list()
            .iter()
            .any(|model| model.id == SMALL.id && model.installed));

        store.remove(SMALL.id).expect("the model is removed");
        assert!(!store.is_installed(&SMALL));
        assert!(!store.dir(SMALL.id).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn installed_spec_reports_a_model_that_is_not_on_disk() {
        let (store, root) = store();
        assert!(matches!(
            store.installed_spec(SMALL.id),
            Err(GlinerError::NotInstalled(_))
        ));
        assert!(matches!(
            store.installed_spec("gliner_nope"),
            Err(GlinerError::UnknownModel { .. })
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_started_download_reports_its_model_and_size() {
        let (_store, root) = store();
        let state = DownloadState::started(&SMALL);
        assert_eq!(state.model, "gliner_small-v2.1");
        assert_eq!(state.total, Some(SMALL.size_bytes()));
        assert!(!state.done);
        assert!(state.error.is_none());

        let dto = state.to_dto();
        assert_eq!(dto.model, state.model);
        assert_eq!(dto.received_bytes, 0);
        assert!(!dto.done);
        let _ = std::fs::remove_dir_all(root);
    }
}
