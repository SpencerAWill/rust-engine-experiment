#![no_std]
#![forbid(unsafe_code)]
//! Typed, generational, copyable identifiers, and containers that hand them out.
//!
//! `engine_genid` is a separate crate because it is `no_std` + `alloc` and
//! dependency-free, because handles cross the C ABI in a later milestone, and
//! because it is the foundation layer for the ECS, assets, and FFI (ADR 0004).
//!
//! # Guarantees
//!
//! - [`GenId`] is 8 bytes (`Option<GenId<T>>` too), `Copy`, and `Send + Sync`
//!   for every `T`. Its bit encoding ([`GenId::to_bits`]) is frozen.
//! - Everything is deterministic: the handles and iteration order a pool produces
//!   are a pure function of the sequence of operations performed on it.
//!
//! # Choosing a container
//!
//! | Need | Use |
//! | --- | --- |
//! | Store values, iterate them contiguously | [`GenIdMap`] |
//! | Only IDs (data lives elsewhere), no iteration | [`GenIdAllocator`] |
//! | Only IDs, and iterate the live ones | [`GenIdMap<()>`](GenIdMap) |
//!
//! Both issue identical handles for the same sequence of operations.
//!
//! # Known limitation
//!
//! A [`GenId<T>`] issued by one pool and used on a *different* pool of the same
//! `T` can resolve to an unrelated element. Detecting this would need extra bits
//! per handle, breaking the 8-byte size and the FFI encoding, or branded
//! lifetimes that make the API much harder to use. Treat a handle as meaningful
//! only for the pool that issued it.
//!
//! # Quick start
//!
//! ```
//! use engine_genid::GenId;
//!
//! struct Enemy;
//! let h = GenId::<Enemy>::from_bits((0 << 32) | 1).unwrap();
//! assert_eq!(GenId::<Enemy>::from_bits(h.to_bits()), Some(h));
//! ```

extern crate alloc;
#[cfg(test)]
extern crate std;

mod allocator;
mod error;
mod id;
mod map;
mod slots;

pub use allocator::GenIdAllocator;
#[cfg(any(test, feature = "test-invariants"))]
pub use error::InvariantViolation;
pub use error::{CapacityError, InvalidGenIdBits};
pub use id::GenId;
pub use map::{Drain, GenIdMap, IntoIter, Iter, IterMut};
