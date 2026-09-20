# Development shell of Alice Voice.
#
# The daemon can read intents with a built in GLiNER model. That model runs
# on the processor, and on a CUDA capable graphics card when the daemon is
# built with the `gliner-cuda` feature.
#
# ONNX Runtime, the engine behind the model, is downloaded by the build.
# Its CUDA build needs the CUDA runtime libraries on the library path,
# because it loads them by name at run time. This shell provides them.
#
#   nix-shell                 # a shell with CUDA libraries
#   nix-shell --arg cuda false  # a shell without CUDA
#
# Inside the shell:
#
#   cd backend
#   cargo run -p alice-daemon --features gliner-cuda
#
# The daemon reports which devices ONNX Runtime offers, so the settings
# page only shows the devices this shell makes possible.
{
  pkgs ? import <nixpkgs> { config.allowUnfree = true; },
  cuda ? true,
  # Use the ONNX Runtime of nixpkgs instead of the one the build
  # downloads. That build carries the processor provider only, so a shell
  # with CUDA leaves this off: the model needs an ONNX Runtime that ships
  # the CUDA provider, and the project downloads a CUDA 12 build of one.
  systemOrt ? false,
}:
let
  inherit (pkgs) lib;

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
  useSystemOrt = systemOrt && lib.versionAtLeast machineOrt.version "1.20";
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
  ] ++ lib.optionals cuda cudaRuntime;

  LD_LIBRARY_PATH = lib.makeLibraryPath (
    with pkgs; [
      openssl
      stdenv.cc.cc.lib
      zlib
    ]
  ) + lib.optionalString cuda (":" + lib.makeLibraryPath cudaRuntime);

  # Read by the settings page and by the build helper of this shell.
  ALICE_GLINER_CUDA = if cuda then "1" else "0";
  ALICE_GLINER_FEATURES = if cuda then "gliner-cuda" else "";

  shellHook = ''
    echo "alice-voice: CUDA ${if cuda then "available" else "off"}${
      lib.optionalString useSystemOrt " (ONNX Runtime from nixpkgs)"
    }"
    ${lib.optionalString cuda ''
      # A CUDA build of the daemon links the ONNX Runtime that the build
      # downloads, and the loader finds a library by name on the library
      # path. Put the CUDA one there, so the model can reach the card.
      ort_dir=$(dirname "$(find "''${XDG_CACHE_HOME:-$HOME/.cache}/ort.pyke.io" \
        -path '*onnxruntime/lib/libonnxruntime_providers_cuda.so' 2>/dev/null | head -n 1)")
      if [ -n "$ort_dir" ] && [ -f "$ort_dir/libonnxruntime.so" ]; then
        export LD_LIBRARY_PATH="$ort_dir:$LD_LIBRARY_PATH"
        echo "alice-voice: ONNX Runtime from $ort_dir"
      else
        echo "alice-voice: no CUDA ONNX Runtime on disk yet"
        echo "alice-voice: build with --features gliner-cuda to fetch one"
      fi
    ''}
  '';
} // lib.optionalAttrs useSystemOrt {
  # The location of a system ONNX Runtime. `ort-sys` links against it
  # instead of downloading one.
  ORT_LIB_LOCATION = "${machineOrt}/lib";
})
