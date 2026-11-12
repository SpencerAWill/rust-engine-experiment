#!/usr/bin/env bash
set -euo pipefail
cargo install --locked cargo-nextest cargo-deny critcmp wasmtime-cli
# Fuzzing: cargo-fuzz, and the pinned nightly toolchain (its date lives only in xtask/src/fuzz.rs).
cargo install --locked cargo-fuzz
rustup toolchain install "$(cargo xtask fuzz-toolchain)" --profile minimal
