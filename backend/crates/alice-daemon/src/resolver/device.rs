//! The device a built in model runs on.
//!
//! Every built in model of the daemon runs through ONNX Runtime, so the
//! GLiNER reader and the local models of the router ask the same question
//! and get the same answer. The runtime reports whether a CUDA or an
//! OpenVINO device is usable right now, and a build reports whether it
//! carries that support, so a request for a device that cannot work fails
//! with a reason the settings page shows instead of running somewhere else
//! in silence.

use std::fmt;

use alice_core::config::LocalDevice;
use ort::execution_providers::{
    CUDAExecutionProvider, ExecutionProvider, OpenVINOExecutionProvider,
};

/// Name of the device that runs on the processor.
pub const DEVICE_CPU: &str = "cpu";

/// Name of the device that runs on a CUDA capable graphics card.
pub const DEVICE_CUDA: &str = "cuda";

/// Name of the device that runs on a GPU via OpenVINO (Intel Arc/NPU) or
/// as a portable alias for any GPU provider.
pub const DEVICE_GPU: &str = "gpu";

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

/// Whether this build carries OpenVINO/GPU support (Intel Arc/NPU).
pub fn gpu_build() -> bool {
    cfg!(any(feature = "gliner-openvino", feature = "gpu"))
}

/// Whether this build carries any GPU support.
pub fn gpu_build_any() -> bool {
    cuda_build() || gpu_build()
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

/// Whether ONNX Runtime can use an OpenVINO device right now.
///
/// On NixOS the system ONNX Runtime is built with OpenVINO GPU/NPU/CPU
/// enabled (`nixpkgs#onnxruntime`), so this returns true on Meteor Lake
/// when `nix-shell --arg gpu true` provides that runtime.
pub fn openvino_available() -> bool {
    OpenVINOExecutionProvider::default()
        .is_available()
        .unwrap_or(false)
}

/// Whether ONNX Runtime can use any GPU device right now.
#[allow(dead_code)]
pub fn gpu_available() -> bool {
    cuda_available() || openvino_available()
}

/// The best GPU device the machine offers right now, CUDA first.
/// Only a device the build carries is considered.
fn best_gpu_device() -> Option<&'static str> {
    if cuda_build() && cuda_available() {
        return Some(DEVICE_CUDA);
    }
    if gpu_build() && openvino_available() {
        return Some(DEVICE_GPU);
    }
    None
}

/// The devices this build and this machine offer, best first.
pub fn available_devices() -> Vec<&'static str> {
    let mut devices = Vec::new();
    if cuda_build() && cuda_available() {
        devices.push(DEVICE_CUDA);
    }
    if gpu_build() && openvino_available() {
        // Expose `gpu` as the portable name; keep `cuda` for existing
        // configs. Avoid duplicating when both are the same hardware.
        if !devices.contains(&DEVICE_GPU) {
            devices.push(DEVICE_GPU);
        }
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
        LocalDevice::Auto => Ok(best_gpu_device().unwrap_or(DEVICE_CPU)),
        LocalDevice::Cuda => {
            if cuda_build() && cuda_available() {
                return Ok(DEVICE_CUDA);
            }
            // Allow `cuda` to fall back to OpenVINO GPU on Intel-only
            // builds where the user stored `cuda` but now runs `gpu`.
            if gpu_build() && openvino_available() {
                return Ok(DEVICE_GPU);
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
        LocalDevice::Gpu => {
            if let Some(device) = best_gpu_device() {
                return Ok(device);
            }
            Err(DeviceError {
                device: DEVICE_GPU.to_string(),
                reason: if gpu_build_any() {
                    "this machine offers no GPU device that ONNX Runtime can use".to_string()
                } else {
                    "this build carries no GPU support, build the daemon with --features gpu (Intel) or --features gliner-cuda (NVIDIA)"
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
        if cuda_build() && cuda_available() {
            assert_eq!(active_device(LocalDevice::Cuda).ok(), Some(DEVICE_CUDA));
            return;
        }
        if gpu_build() && openvino_available() {
            // On Intel GPU builds `cuda` falls back to `gpu`.
            assert_eq!(active_device(LocalDevice::Cuda).ok(), Some(DEVICE_GPU));
            return;
        }
        let err = active_device(LocalDevice::Cuda).expect_err("this machine has no CUDA device");
        assert!(err.reason.contains("cuda"), "{err}");
        assert!(
            err.reason.contains("--features gliner-cuda") || err.reason.contains("no CUDA device"),
            "{err}"
        );
    }

    #[test]
    fn a_gpu_request_reports_why_it_cannot_work() {
        if gpu_build() && openvino_available() || cuda_build() && cuda_available() {
            assert!(
                available_devices().contains(&DEVICE_GPU)
                    || available_devices().contains(&DEVICE_CUDA)
            );
            let device = active_device(LocalDevice::Gpu).expect("gpu available");
            assert!(device == DEVICE_GPU || device == DEVICE_CUDA);
            return;
        }
        let err = active_device(LocalDevice::Gpu).expect_err("this machine has no GPU device");
        let lower = err.reason.to_lowercase();
        assert!(lower.contains("gpu"), "{err}");
        assert!(
            lower.contains("--features gpu") || lower.contains("no gpu device"),
            "{err}"
        );
    }
}
