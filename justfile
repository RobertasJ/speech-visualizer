default:
    @just --list

# Debug build (CPU)
build:
    cargo build

# Release build (CPU)
release:
    cargo build --release

# Run on CPU; extra args go to the binary
run *args:
    cargo run --release -- {{args}}

# Release build with CUDA
build-cuda:
    cargo build --release --features cuda

# Run with CUDA
run-cuda *args:
    cargo run --release --features cuda -- {{args}}

# Release build with Vulkan
build-vulkan:
    cargo build --release --features vulkan

# Run with Vulkan
run-vulkan *args:
    cargo run --release --features vulkan -- {{args}}

check:
    cargo check

clippy:
    cargo clippy --all-targets

fmt:
    cargo fmt

clean:
    cargo clean

# Bump nixpkgs in flake.lock
update:
    nix flake update
