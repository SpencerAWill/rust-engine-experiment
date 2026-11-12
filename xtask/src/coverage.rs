//! `cargo xtask coverage`: line and function coverage through `cargo-llvm-cov`.

#[expect(
    dead_code,
    reason = "the summary is wired into the command in the next step"
)]
mod summary;
