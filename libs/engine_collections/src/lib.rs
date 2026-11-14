#![no_std]
#![forbid(unsafe_code)]
//! Deterministic insertion-ordered hash map and set.
//!
//! `engine_collections` is a separate crate for the reasons in ADR 0004: its
//! dependencies (`indexmap` and `foldhash`) should not be carried by other
//! crates; it is `no_std` with `alloc`; and it is a foundation layer for the
//! ECS, the registry and persistence.
//!
//! [`IndexMap`] and [`IndexSet`] iterate in insertion order, so iteration
//! order is a pure function of the operations applied. The hashing algorithm
//! and its seed are not part of the contract and may change in any release.
//!
//! # Guarantees
//!
//! - **Order is a function of the operation sequence.** A new key goes at the
//!   end; inserting an existing key keeps its position; `swap_remove` moves the
//!   last entry into the hole; `shift_remove` moves later entries up one;
//!   `pop`, `retain` and `clear` keep the relative order of what remains;
//!   `sort_keys` (map) and `sort` (set) put the keys in `Ord` order.
//! - **It does not depend on hash values, the seed, capacity, pointer width,
//!   platform, or the versions of `indexmap` and `foldhash`**, and not on the
//!   *values* of the keys either: keys that differ between runs (addresses
//!   under ASLR, `TypeId` across builds) still iterate in a deterministic
//!   order. `BTreeMap` does not have that property.
//! - **Positions are not stable.** A removal or sort changes the positions of
//!   other entries; a position from `get_index_of` is valid only until the next
//!   removal or sort.
//! - **Equality respects order.** Two collections are equal only if they hold
//!   the same entries in the same order, and `Hash` agrees. To use a set as an
//!   order-independent key, sort it first.
//! - **Heap use follows ADR 0011.** After `with_capacity(n)` or `reserve(n)`,
//!   the next `n` insertions of new keys, and every lookup, iteration, removal,
//!   `retain`, `clear` and sort, do not allocate.
//!
//! # Your part
//!
//! The caller owns a deterministic operation sequence. Inserting from several
//! threads in completion order gives a different order on every run, and no
//! hasher can fix that: merge such work in a fixed order first.
//!
//! # Choosing a collection
//!
//! | Need | Use |
//! | --- | --- |
//! | Hashed lookup, iteration in insertion order | [`IndexMap`], [`IndexSet`] |
//! | Iteration in sorted key order | `BTreeMap`, `BTreeSet` |
//! | A few order-independent keys | a sorted `Vec` |
//!
//! std's `HashMap` and `HashSet` are banned in engine code (`clippy.toml`):
//! their iteration order is randomized per process.
//!
//! # Known limitation
//!
//! The seed is fixed and public, so keys chosen by an attacker could make
//! every lookup O(n). Nothing in the engine hashes untrusted keys yet; a
//! per-world seed drawn from the world's `Prng` is deferred. If a key's
//! `Hash`, `Eq` or `Ord` is inconsistent or panics, results are unspecified
//! but memory-safe, as with std's collections.
//!
//! # Quick start
//!
//! ```
//! use engine_collections::IndexMap;
//!
//! let mut speeds = IndexMap::new();
//! speeds.insert("walk", 1.4);
//! speeds.insert("run", 4.0);
//! speeds.insert("crawl", 0.3);
//! speeds.insert("walk", 1.5); // existing key: value replaced, position kept
//!
//! assert_eq!(speeds.keys().copied().collect::<Vec<_>>(), ["walk", "run", "crawl"]);
//! assert_eq!(speeds["walk"], 1.5);
//! assert_eq!(speeds.get_index_of("run"), Some(1));
//!
//! speeds.swap_remove("walk"); // the last entry moves into the hole
//! assert_eq!(speeds.keys().copied().collect::<Vec<_>>(), ["crawl", "run"]);
//! ```

#[cfg(test)]
#[macro_use]
extern crate std;

mod error;
mod hasher;
pub mod index_map;
pub mod index_set;

pub use error::TryReserveError;
pub use index_map::IndexMap;
pub use index_set::IndexSet;

#[cfg(test)]
mod tests;
