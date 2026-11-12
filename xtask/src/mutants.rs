//! `cargo xtask mutants`: mutation testing through `cargo-mutants`.

// The pure parts land before the command that uses them; this goes when the command is wired in.
#![allow(dead_code)]

mod excludes;
