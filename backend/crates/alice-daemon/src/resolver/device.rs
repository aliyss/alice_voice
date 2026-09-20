//! The device a built in model runs on.
//!
//! Every built in model of the daemon runs through ONNX Runtime, so the
//! GLiNER reader and the local models of the router ask the same question
//! and get the same answer. The runtime reports whether a CUDA device is
//! usable right now, and a build reports whether it carries CUDA support,
//! so a request for a device that cannot work fails with a reason the
//! settings page shows instead of running somewhere else in silence.

use std::fmt;

use alice_core::config::LocalDevice;
use ort::execution_providers::{CUDAExecutionProvider, ExecutionProvider};

/// Name of the device that runs on the processor.
pub const DEVICE_CPU: &str = "cpu";

/// Name of the device that runs on a CUDA capable graphics card.
pub const DEVICE_CUDA: &str = "cuda";

/// Why a device cannot run a built in model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceError {
    /// Device the caller asked for.
    pub device: String,
    /// Why the device is not usable.
    pub reason: String,
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot run on {}: {}", self.device, self.reason)
    }
}

impl std::error::Error for DeviceError {}

/// Whether this build carries CUDA support.
pub fn cuda_build() -> bool {
    cfg!(feature = "gliner-cuda")
}

/// Whether ONNX Runtime can use a CUDA device right now.
///
/// The check asks the runtime itself, so a build with CUDA support on a
/// machine without a usable device reports false.
pub fn cuda_available() -> bool {
    CUDAExecutionProvider::default()
        .is_available()
        .unwrap_or(false)
}

/// The devices this build and this machine offer, best first.
pub fn available_devices() -> Vec<&'static str> {
    let mut devices = Vec::new();
    if cuda_available() {
        devices.push(DEVICE_CUDA);
    }
    devices.push(DEVICE_CPU);
    devices
}

/// The device one preference runs on.
///
/// `auto` takes the best device the build and the machine offer. A request
/// for a device that cannot work fails with a reason the settings page can
/// show, instead of silently running somewhere else.
pub fn active_device(preference: LocalDevice) -> Result<&'static str, DeviceError> {
    match preference {
        LocalDevice::Cpu => Ok(DEVICE_CPU),
        LocalDevice::Auto => Ok(if cuda_available() {
            DEVICE_CUDA
        } else {
            DEVICE_CPU
        }),
        LocalDevice::Cuda => {
            if cuda_available() {
                return Ok(DEVICE_CUDA);
            }
            Err(DeviceError {
                device: DEVICE_CUDA.to_string(),
                reason: if cuda_build() {
                    "this machine offers no CUDA device that ONNX Runtime can use".to_string()
                } else {
                    "this build carries no CUDA support, build the daemon with --features gliner-cuda"
                        .to_string()
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_processor_is_always_available() {
        let devices = available_devices();
        assert!(devices.contains(&DEVICE_CPU));
        assert_eq!(active_device(LocalDevice::Cpu).ok(), Some(DEVICE_CPU));
    }

    #[test]
    fn auto_never_fails() {
        let device = active_device(LocalDevice::Auto).expect("auto always picks a device");
        assert!(available_devices().contains(&device));
    }

    #[test]
    fn a_cuda_request_reports_why_it_cannot_work() {
        if cuda_available() {
            assert_eq!(active_device(LocalDevice::Cuda).ok(), Some(DEVICE_CUDA));
            return;
        }
        let err = active_device(LocalDevice::Cuda).expect_err("this machine has no CUDA device");
        assert!(err.reason.contains("cuda"), "{err}");
        assert!(
            err.reason.contains("--features gliner-cuda") || err.reason.contains("no CUDA device"),
            "{err}"
        );
    }
}
