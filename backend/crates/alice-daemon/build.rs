//! Build helper of the daemon.
//!
//! The built in GLiNER model links an ONNX Runtime. The bindings link the
//! one of the machine when `ORT_LIB_LOCATION` names it, and otherwise link
//! the one they download. A layout of the machine carries its own run
//! path, but a download lives in a cache directory, and the loader finds a
//! library by name only on the run path. This helper adds that directory
//! to the run path of the binary and of every test binary, so a
//! `cargo run` and a `cargo test` work outside the development shell.
//!
//! A CUDA build needs the CUDA libraries of the machine at run time as
//! well. Those come from `shell.nix`, which no run path can carry.
//!
//! A set of the bindings that ships an archive instead of a shared library
//! needs nothing here: the linker puts the runtime into the binary.

use std::path::{Path, PathBuf};

/// The variable that names the ONNX Runtime of the machine.
const LIB_LOCATION: &str = "ORT_LIB_LOCATION";

/// The name of the linked library.
const LIB_NAME: &str = "libonnxruntime.so";

/// The name of the provider that only a CUDA build of the runtime has.
const CUDA_PROVIDER: &str = "libonnxruntime_providers_cuda.so";

/// How deep the search walks the cache of the bindings.
const MAX_DEPTH: usize = 5;

fn main() {
    println!("cargo:rerun-if-env-changed={LIB_LOCATION}");
    if std::env::var_os(LIB_LOCATION).is_some() {
        // A layout of the machine reports its own run path.
        return;
    }

    // A build without a shared runtime needs no run path.
    let Some(dir) = downloaded_runtime() else {
        return;
    };

    let dir = dir.display();
    println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
}

/// The directory of a shared ONNX Runtime that the bindings downloaded.
///
/// The bindings download one set per feature set. A build with the CUDA
/// feature links the set that carries the CUDA provider, and a build
/// without it links the set that does not. The processor set of a release
/// is an archive, which needs no run path and leaves this empty.
fn downloaded_runtime() -> Option<PathBuf> {
    let root = cache_root()?.join("ort.pyke.io");
    let cuda = std::env::var_os("CARGO_FEATURE_GLINER_CUDA").is_some();

    let mut found = Vec::new();
    collect(&root, 0, &mut found);
    found
        .into_iter()
        .find(|dir| dir.join(CUDA_PROVIDER).is_file() == cuda)
}

/// The cache directory of the machine.
fn cache_root() -> Option<PathBuf> {
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(cache));
    }
    std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache"))
}

/// Collect every directory below `path` that holds the linked library.
fn collect(path: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || !path.is_dir() {
        return;
    }
    if path.join(LIB_NAME).is_file() {
        found.push(path.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect(&entry.path(), depth + 1, found);
    }
}
