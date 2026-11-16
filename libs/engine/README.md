# libs/engine/

Status: Living (2026-11-15). The `engine` crate: the only library crate ([ADR 0004](../../docs/adr/0004-crate-splitting-rule.md)). Its modules and what each is for are in the [root README](../../README.md#module-map); the API reference is its rustdoc (`cargo doc --open`).

## When this applies

- You add or change library code, its tests or its benchmarks.

The rules for library code are in [CONTRIBUTING.md](../../CONTRIBUTING.md#what-ci-enforces) ("What CI enforces": one crate, no `unsafe`, determinism, no global state, frozen formats, documentation and panics) and its [Tests](../../CONTRIBUTING.md#tests) section. A change here can move an instruction count: run the [gate](../../bench/README.md#instruction-count-gate). Follow the lessons in [docs/lessons/](../../docs/lessons/README.md) whose `applies_to` matches the files you change.

## Contents

- `Cargo.toml`: features `std` (default; `time::SystemClock`, and `std` as the transcendental backend of `math::scalar`) and `test-invariants` (exposes `check_invariants` to integration tests and fuzz targets).
- `src/`: `lib.rs` (crate root and `prelude`) and one module per file, each with a folder of the same name for its submodules and inline unit tests: `genid`, `collections`, `random`, `time`, `math`.
- `tests/`: integration tests, named `<module>_<kind>.rs` (contracts, panics, model and property tests, golden values, allocation and auto-trait checks).
- `benches/`: Criterion benchmarks that need crate internals; the comparisons and the gate's benchmarks are in [bench/](../../bench/README.md).
