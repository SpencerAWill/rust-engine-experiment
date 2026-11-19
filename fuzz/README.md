# fuzz/

Status: Living (2026-11-15). Coverage-guided fuzz targets, run with `cargo-fuzz` on a pinned nightly; outside the workspace.

## When this applies

- You run a fuzz target, or the nightly `fuzz` job went red.
- You add a target, or bump the pinned nightly.

## Contents

- `fuzz_targets/`: one file per target. `genid_map_ops` and `genid_allocator_ops` drive `GenIdMap` and `GenIdAllocator` with arbitrary operation sequences against a simple reference model, checking `check_invariants` after every operation; `genid_from_bits` checks `GenId::from_bits`, `UntypedGenId::from_bits` and `to_bits`; `bit_set_ops` drives `BitSet`; `hierarchical_bit_set_ops` drives `HierarchicalBitSet` and `Join`.
- `Cargo.toml`: the fuzz package, outside the workspace.
- `Cargo.lock`: its own lockfile, which Dependabot watches.
- `.gitignore`: keeps `artifacts/`, `corpus/` and `target/` out of git.

## How to

```sh
cargo xtask fuzz genid_map_ops --time 60      # one target for 60 s (the default)
cargo xtask fuzz all                            # every target, 10 min each, as the nightly job does
cargo xtask fuzz genid_map_ops --replay fuzz/artifacts/genid_map_ops/crash-<hash>
cargo xtask fuzz-build                          # compile only; CI does this on every pull request
```

You need `cargo-fuzz` and the pinned nightly; the command prints the install lines when either is missing. Fuzzing does not work on Windows (libFuzzer needs a non-MSVC toolchain): use the devcontainer or WSL. `cargo xtask ci` skips `fuzz-build` with a loud note when the tools are missing, but fails when `CI` is set.

- **A finding.** The nightly `fuzz` job goes red and uploads `fuzz/artifacts` for 30 days. Turn the crash into a failing test before fixing it ([Tests](../CONTRIBUTING.md#tests)). Do not commit crash files or the corpus.
- **Bumping the nightly.** Change `FUZZ_NIGHTLY` in `xtask/src/fuzz.rs` (the only place the date is written; CI and the devcontainer read it through `cargo xtask fuzz-toolchain`), run `cargo xtask fuzz-build`, and mention the bump in the pull request.
- **Why `fuzz/` is outside `cargo xtask deny`.** It is a developer tool that is never built into or shipped with an engine artifact, and `libfuzzer-sys` is licensed `NCSA` (in addition to MIT or Apache-2.0), which is outside the allow list of [ADR 0002](../docs/adr/0002-originality-and-dependency-licensing.md). Dependabot watches `fuzz/Cargo.lock` instead, so advisories on its dependencies still surface.
