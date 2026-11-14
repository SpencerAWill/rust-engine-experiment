#![forbid(unsafe_code)]
// benchmark harness, not simulation code
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used
)]
//! Cross-library comparison benchmarks for the engine's foundation crates.
//!
//! This crate has no library code. The benchmarks live in `benches/` and compare
//! the engine's types against well-known alternatives (`slotmap`, `thunderdome`).
//! Run them with `cargo bench -p engine_benches`, or `cargo xtask bench`.
