#![no_std]
#![forbid(unsafe_code)]
//! Typed, generational, copyable identifiers, and containers that hand them out.
//!
//! `engine_handle` is a separate crate because it is `no_std` + `alloc` and
//! dependency-free, because handles cross the C ABI in a later milestone, and
//! because it is the foundation layer for the ECS, assets, and FFI (ADR 0004).
//!
//! # Guarantees
//!
//! - [`Handle`] is 8 bytes (`Option<Handle<T>>` too), `Copy`, and `Send + Sync`
//!   for every `T`. Its bit encoding ([`Handle::to_bits`]) is frozen.
//! - Everything is deterministic: the handles and iteration order a pool produces
//!   are a pure function of the sequence of operations performed on it.
//!
//! # Known limitation
//!
//! A [`Handle<T>`] issued by one pool and used on a *different* pool of the same
//! `T` can resolve to an unrelated element. Detecting this would need extra bits
//! per handle, breaking the 8-byte size and the FFI encoding, or branded
//! lifetimes that make the API much harder to use. Treat a handle as meaningful
//! only for the pool that issued it.
//!
//! # Quick start
//!
//! ```
//! use engine_handle::Handle;
//!
//! struct Enemy;
//! let h = Handle::<Enemy>::from_bits((0 << 32) | 1).unwrap();
//! assert_eq!(Handle::<Enemy>::from_bits(h.to_bits()), Some(h));
//! ```

// Becomes unconditional with the pool (Task 6).
#[cfg(test)]
extern crate alloc;

mod handle;

pub use handle::Handle;
