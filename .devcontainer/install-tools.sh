#!/usr/bin/env bash
set -euo pipefail
cargo install --locked cargo-nextest cargo-deny critcmp wasmtime-cli
# Fuzzing: cargo-fuzz, and the pinned nightly toolchain (its date lives only in xtask/src/fuzz.rs).
# Optional, so a failure here (no network, say) must not abort the container setup.
{
  cargo install --locked cargo-fuzz &&
    rustup toolchain install "$(cargo xtask fuzz-toolchain)" --profile minimal
} || echo "warning: fuzzing tools were not installed; run the two commands above by hand" >&2
# Coverage: cargo-llvm-cov and the LLVM tools it needs. Optional, like the fuzzing tools.
{
  cargo install --locked cargo-llvm-cov &&
    rustup component add llvm-tools-preview
} || echo "warning: coverage tools were not installed; run the two commands above by hand" >&2
# Mutation testing: cargo-mutants, at the version pinned as MUTANTS_VERSION in xtask/src/mutants.rs
# (a unit test checks they agree). Optional, like the tools above.
cargo install --locked cargo-mutants --version 27.1.0 ||
  echo "warning: cargo-mutants was not installed; run the command above by hand" >&2
