//! Compile-time layout assertions.
//!
//! `const` assertions pin `size_of` and `align_of` of the transform types (added with the
//! types themselves, in later tasks), so that anything that silently changes their layout, such
//! as a glam `scalar-math` feature flip through Cargo feature unification or a glam upgrade,
//! fails the build loudly instead of changing snapshot or FFI layouts (ADR 0003, ADR 0006).
