//! Inference of the built in GLiNER models.
//!
//! One loaded model serves every message. Loading costs about a second and
//! one inference stays under ten milliseconds on a processor, so the engine
//! keeps the session and reloads it only when the model or the device
//! changes.
//!
//! Inference is blocking work, so the resolver runs it on the blocking
//! pool of the runtime. A slow turn must not stall the socket stream.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use alice_core::config::LocalDevice;
use gliner::model::input::text::TextInput;
use gliner::model::params::Parameters;
use gliner::model::pipeline::span::SpanMode;
use gliner::model::GLiNER;
use orp::params::RuntimeParameters;
use ort::execution_providers::CUDAExecutionProvider;
use ort::execution_providers::ExecutionProviderDispatch;

pub use crate::resolver::device::DEVICE_CPU;

use crate::resolver::device::{self, DEVICE_CUDA};
use crate::resolver::gliner::catalog::ModelSpec;
use crate::resolver::gliner::error::GlinerError;
use crate::resolver::gliner::store::GlinerStore;

/// Smallest probability the session keeps.
///
/// The reader applies the threshold the user configured, and the floor
/// here keeps the near misses on hand, so the daemon can report what a
/// message almost matched.
const SESSION_THRESHOLD: f32 = 0.05;

/// One label the model found in a message.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelHit {
    /// The label the span matched.
    pub label: String,
    /// The text of the span.
    pub text: String,
    /// Probability of the span, between 0 and 1.
    pub score: f32,
}

/// Whether this build carries CUDA support.
pub fn cuda_build() -> bool {
    device::cuda_build()
}

/// The devices this build and this machine offer, best first.
pub fn available_devices() -> Vec<&'static str> {
    device::available_devices()
}

/// The device one preference runs on.
///
/// `auto` takes the best device the build and the machine offer. A request
/// for a device that cannot work fails with a reason the settings page can
/// show, instead of silently running somewhere else.
pub fn active_device(preference: LocalDevice) -> Result<&'static str, GlinerError> {
    device::active_device(preference).map_err(|err| GlinerError::Device {
        device: err.device,
        reason: err.reason,
    })
}

/// One loaded model.
struct Loaded {
    /// Identifier of the model.
    model: String,
    /// The device the session runs on.
    device: &'static str,
    /// The session that answers every message.
    session: GLiNER<SpanMode>,
}

impl fmt::Debug for Loaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Loaded")
            .field("model", &self.model)
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

/// The engine that runs the built in GLiNER models.
#[derive(Clone, Debug)]
pub struct GlinerEngine {
    store: Arc<GlinerStore>,
    threads: usize,
    loaded: Arc<Mutex<Option<Loaded>>>,
}

impl GlinerEngine {
    /// Create a new engine.
    pub fn new(store: Arc<GlinerStore>, threads: usize) -> Self {
        Self {
            store,
            threads: threads.max(1),
            loaded: Arc::new(Mutex::new(None)),
        }
    }

    /// The store the engine reads its models from.
    pub fn store(&self) -> &Arc<GlinerStore> {
        &self.store
    }

    /// Run one inference and report every span the model found.
    ///
    /// This blocks on the model, so the caller runs it on the blocking
    /// pool of the runtime.
    pub fn infer(
        &self,
        spec: &'static ModelSpec,
        device: LocalDevice,
        text: &str,
        labels: &[String],
    ) -> Result<Vec<LabelHit>, GlinerError> {
        let device = active_device(device)?;
        if labels.is_empty() {
            return Ok(Vec::new());
        }
        let mut loaded = self.lock();
        let current = loaded
            .as_ref()
            .is_some_and(|loaded| loaded.model == spec.id && loaded.device == device);
        if !current {
            *loaded = Some(self.load(spec, device)?);
        }
        let loaded = loaded
            .as_ref()
            .ok_or_else(|| GlinerError::Load("the model did not load".to_string()))?;
        run(&loaded.session, text, labels)
    }

    /// Load one model onto one device.
    fn load(&self, spec: &'static ModelSpec, device: &'static str) -> Result<Loaded, GlinerError> {
        let runtime = match device {
            DEVICE_CUDA => {
                RuntimeParameters::new(self.threads, [CUDAExecutionProvider::default().build()])
            }
            _ => RuntimeParameters::new(self.threads, Vec::<ExecutionProviderDispatch>::new()),
        };
        let params = Parameters::default().with_threshold(SESSION_THRESHOLD);
        let dir = self.store.dir(spec.id);
        let tokenizer = dir.join("tokenizer.json");
        let graph = file_with_suffix(spec, ".onnx")
            .map(|file| dir.join(file.path))
            .ok_or_else(|| GlinerError::Load(format!("{} carries no graph", spec.id)))?;

        let session = GLiNER::<SpanMode>::new(params, runtime, &tokenizer, &graph)
            .map_err(|err| GlinerError::Load(err.to_string()))?;
        tracing::info!(
            model = spec.id,
            device,
            threads = self.threads,
            "the built in GLiNER model is ready"
        );
        Ok(Loaded {
            model: spec.id.to_string(),
            device,
            session,
        })
    }

    /// Lock the loaded model.
    fn lock(&self) -> MutexGuard<'_, Option<Loaded>> {
        self.loaded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Find the file of one model that ends with a suffix.
fn file_with_suffix(
    spec: &'static ModelSpec,
    suffix: &str,
) -> Option<&'static crate::resolver::gliner::catalog::ModelFile> {
    spec.files.iter().find(|file| file.path.ends_with(suffix))
}

/// Run one inference on a loaded session.
fn run(
    session: &GLiNER<SpanMode>,
    text: &str,
    labels: &[String],
) -> Result<Vec<LabelHit>, GlinerError> {
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let input = TextInput::from_str(&[text], &labels)
        .map_err(|err| GlinerError::Inference(err.to_string()))?;
    let output = session
        .inference(input)
        .map_err(|err| GlinerError::Inference(err.to_string()))?;

    let mut hits: Vec<LabelHit> = Vec::new();
    for spans in &output.spans {
        for span in spans {
            hits.push(LabelHit {
                label: span.class().to_string(),
                text: span.text().to_string(),
                score: span.probability(),
            });
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::gliner::catalog::SMALL;

    #[test]
    fn the_processor_is_always_available() {
        let devices = available_devices();
        assert!(devices.contains(&device::DEVICE_CPU));
        assert_eq!(
            active_device(LocalDevice::Cpu).ok(),
            Some(device::DEVICE_CPU)
        );
    }

    #[test]
    fn a_cuda_request_reports_why_it_cannot_work() {
        if device::cuda_available() {
            assert_eq!(active_device(LocalDevice::Cuda).ok(), Some(DEVICE_CUDA));
            return;
        }
        let err = active_device(LocalDevice::Cuda).expect_err("this machine has no CUDA device");
        let message = err.to_string();
        assert!(message.contains("cuda"), "{message}");
        assert!(
            message.contains("--features gliner-cuda") || message.contains("no CUDA device"),
            "{message}"
        );
    }

    #[test]
    fn the_graph_file_is_the_onnx_file() {
        let file = file_with_suffix(&SMALL, ".onnx").expect("the model carries a graph");
        assert!(file.path.ends_with(".onnx"));
    }
}
