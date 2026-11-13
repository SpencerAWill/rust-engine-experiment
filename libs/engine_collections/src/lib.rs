#![no_std]
#![forbid(unsafe_code)]
//! Deterministic insertion-ordered hash map and set.
//!
//! `engine_collections` is a separate crate for the reasons in ADR 0004: its
//! dependencies (`indexmap` and `foldhash`) should not be carried by other
//! crates; it is `no_std` with `alloc`; and it is a foundation layer for the
//! ECS, the registry and persistence.
//!
//! The hashing algorithm and its seed are not part of the contract and may
//! change in any release.

#[cfg(test)]
#[macro_use]
extern crate std;

mod error;
mod hasher;
pub mod index_map;

pub use error::TryReserveError;
pub use index_map::IndexMap;

#[cfg(test)]
mod tests;
