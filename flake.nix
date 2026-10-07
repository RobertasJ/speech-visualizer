{
  description = "speech-visualizer dev shell (whisper-rs with CUDA / Vulkan)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true; # CUDA
      };
      cuda = pkgs.cudaPackages.cudatoolkit;
      # Freya (from its flake.nix): window, GL rendering and fonts. Most are dlopen'ed at
      # runtime, so they also go on LD_LIBRARY_PATH. Its GTK/WebKit libs are left out;
      # they're only for the tray and webview features.
      freyaLibraries = with pkgs; [
        fontconfig
        freetype
        libGL
        libxkbcommon
        wayland
        libx11
        libxcursor
        libxi
        libxrandr
      ];
    in
    {
      devShells.${system}.default =
        (pkgs.mkShell.override { stdenv = pkgs.cudaPackages.backendStdenv; }) {
          nativeBuildInputs = with pkgs; [
            cmake
            pkg-config
            rustPlatform.bindgenHook # sets LIBCLANG_PATH for whisper-rs-sys bindgen
            cuda
            shaderc # glslc, for the vulkan feature
          ];
          buildInputs = with pkgs; [
            alsa-lib # cpal audio backend
            vulkan-headers
            vulkan-loader
          ] ++ freyaLibraries;

          CUDA_PATH = cuda;
          CUDAToolkit_ROOT = cuda;
          # RTX 3070 Ti = sm_86; building only this arch cuts whisper.cpp compile time a lot.
          CMAKE_CUDA_ARCHITECTURES = "86";
          VULKAN_SDK = pkgs.vulkan-loader;
          # whisper-rs-sys only searches /usr/local/cuda and /opt/cuda for link libs.
          RUSTFLAGS = "-L ${cuda}/lib -L ${cuda}/lib/stubs";
          # Real libcuda.so comes from the NixOS driver at runtime.
          LD_LIBRARY_PATH = "/run/opengl-driver/lib:${pkgs.lib.makeLibraryPath freyaLibraries}";
        };
    };
}
