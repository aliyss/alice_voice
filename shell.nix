# Development shell of Alice Voice.
#
# The daemon can read intents with a built in GLiNER model. That model runs
# on the processor, and on a graphics device when the daemon is built with
# an accelerator feature.
#
# ONNX Runtime, the engine behind the model, is downloaded by the build.
# Its CUDA build needs the CUDA runtime libraries on the library path,
# because it loads them by name at run time. This shell provides them.
#
# For Intel Arc / NPU (Meteor Lake, this laptop) the daemon uses OpenVINO
# via a system ONNX Runtime from nixpkgs (built with OpenVINO).
#
#   nix-shell                              # CPU only (default)
#   nix-shell --arg cuda true              # CUDA (NVIDIA) with downloaded ORT
#   nix-shell --arg gpu true               # Intel GPU/NPU via OpenVINO (system ORT)
#   nix-shell --arg openvino true          # alias for gpu
#   nix-shell --arg gpu true --arg cuda true  # both (e.g. hybrid machine)
#
# Inside the shell:
#
#   cd backend
#   cargo run -p alice-daemon                          # CPU only (default)
#   cargo run -p alice-daemon --features gliner-cuda   # with CUDA (requires --arg cuda true)
#   cargo run -p alice-daemon --features gpu           # with Intel GPU (requires --arg gpu true)
#   cargo run -p alice-daemon --features gliner-openvino  # same as gpu
#
# The daemon reports which devices ONNX Runtime offers, so the settings
# page only shows the devices this shell makes possible.
{
  pkgs ? import <nixpkgs> { config.allowUnfree = true; },
  cuda ? false,
  gpu ? false,
  openvino ? false,
  # Use the ONNX Runtime of nixpkgs instead of the one the build
  # downloads. That build carries the processor provider only, so a shell
  # with CUDA leaves this off: the model needs an ONNX Runtime that ships
  # the CUDA provider, and the project downloads a CUDA 12 build of one.
  # For OpenVINO/GPU the system ORT is required (nixpkgs onnxruntime is
  # built with -Donnxruntime_USE_OPENVINO=ON).
  systemOrt ? false,
}:
let
  inherit (pkgs) lib;

  # `gpu` is the portable alias, `openvino` is the explicit Intel name.
  gpuEnabled = gpu || openvino;

  # The CUDA build of ONNX Runtime is a CUDA 12 build, and cuDNN 9
  # belongs to CUDA 12.
  cudaPackages = pkgs.cudaPackages_12 or pkgs.cudaPackages;

  # The libraries ONNX Runtime loads by name at run time.
  cudaRuntime = with cudaPackages; [
    cuda_cudart
    cudatoolkit
    cudnn
  ];

  # An ONNX Runtime of the machine replaces the downloaded one, but the
  # Rust bindings need 1.20 or newer and nixpkgs may carry an older one.
  # A build that is too old is not linked, so the build downloads its own.
  machineOrt = pkgs.onnxruntime;
  # GPU/OpenVINO always needs the system ORT (downloaded binaries are
  # `none` or `cu12` only, from `ort-sys/dist.txt`, and don't carry
  # OpenVINO).
  useSystemOrt = (systemOrt || gpuEnabled) && lib.versionAtLeast machineOrt.version "1.20";
in
pkgs.mkShell ({
  name = "alice-voice";

  packages = with pkgs; [
    cargo
    clippy
    cmake
    curl
    openssl
    pkg-config
    rustc
    rustfmt
    zlib
  ] ++ lib.optionals cuda cudaRuntime
    ++ lib.optionals gpuEnabled [ pkgs.openvino pkgs.onnxruntime ];

  LD_LIBRARY_PATH = lib.makeLibraryPath (
    [ pkgs.openssl pkgs.stdenv.cc.cc.lib pkgs.zlib ]
    ++ lib.optionals gpuEnabled [ pkgs.openvino pkgs.onnxruntime ]
  ) + lib.optionalString cuda (":" + lib.makeLibraryPath cudaRuntime)
    + lib.optionalString gpuEnabled (":" + lib.makeLibraryPath [ pkgs.openvino pkgs.onnxruntime ]);

  # Read by the settings page and by the build helper of this shell.
  ALICE_GLINER_CUDA = if cuda then "1" else "0";
  ALICE_GLINER_GPU = if gpuEnabled then "1" else "0";
  ALICE_GLINER_FEATURES =
    lib.concatStringsSep "," (
      lib.optionals cuda [ "gliner-cuda" ] ++ lib.optionals gpuEnabled [ "gpu" ]
    );

  shellHook = ''
    echo "alice-voice: CUDA ${if cuda then "available" else "off"}, GPU/OpenVINO ${if gpuEnabled then "available" else "off"}${lib.optionalString useSystemOrt " (ONNX Runtime from nixpkgs)"}"
    ${lib.optionalString cuda ''
      # A CUDA build of the daemon links the ONNX Runtime that the build
      # downloads, and the loader finds a library by name on the library
      # path. Put the CUDA one there, so the model can reach the card.
      ort_dir=$(dirname "$(find "''${XDG_CACHE_HOME:-$HOME/.cache}/ort.pyke.io" \
        -path '*onnxruntime/lib/libonnxruntime_providers_cuda.so' 2>/dev/null | head -n 1)")
      if [ -n "$ort_dir" ] && [ -f "$ort_dir/libonnxruntime.so" ]; then
        export LD_LIBRARY_PATH="$ort_dir:$LD_LIBRARY_PATH"
        echo "alice-voice: ONNX Runtime (CUDA) from $ort_dir"
      else
        echo "alice-voice: no CUDA ONNX Runtime on disk yet"
        echo "alice-voice: build with --features gliner-cuda to fetch one"
      fi
    ''}
    ${lib.optionalString gpuEnabled ''
      echo "alice-voice: OpenVINO GPU available via system ONNX Runtime (${
        if useSystemOrt then "${machineOrt}/lib" else "download"
      })"
      ${lib.optionalString (!useSystemOrt) ''
        echo "alice-voice: warning: system ONNX Runtime not used (need nixpkgs >= 1.20), GPU will not be available"
      ''}
    ''}
  '';
} // lib.optionalAttrs useSystemOrt {
  # The location of a system ONNX Runtime. `ort-sys` links against it
  # instead of downloading one.
  ORT_LIB_LOCATION = "${machineOrt}/lib";
})
