#![forbid(unsafe_code)]
// benchmark harness, not simulation code
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used
)]
//! Instruction-count regression benchmarks for the engine's foundation crates.
//!
//! This crate has no library code. The benchmarks live in `benches/` and run under Valgrind
//! through Gungraun, which counts CPU instructions instead of timing them, so the numbers do
//! not depend on the machine. Each benchmark measures one operation at one small, fixed size.
//!
//! Do not run them with `cargo bench`: use `cargo xtask bench-regress`, which compares the
//! counts of two commits and decides whether the change regressed. Valgrind only works on
//! Linux, so on other systems run it in the devcontainer. `cargo xtask bench-smoke` runs every
//! benchmark function once, without Valgrind, to check that they still work.
