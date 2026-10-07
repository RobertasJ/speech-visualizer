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
    cargo run -- {{args}}

# Build with CUDA
build-cuda:
    cargo build --features cuda

# Run with CUDA
run-cuda *args:
    cargo run --features cuda -- {{args}}

# Build with Vulkan
build-vulkan:
    cargo build --features vulkan

# Run with Vulkan
run-vulkan *args:
    cargo run --features vulkan -- {{args}}

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

update-state:
    @jj git fetch
    @snapshot=$(jj log -r @ --no-graph -T 'commit_id'); \
    jj rebase -r @ -d main; \
    jj restore --from "$snapshot"

