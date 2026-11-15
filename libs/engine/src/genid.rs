//! Generational IDs: typed, copyable identifiers that go stale when their value is removed, and
//! the containers that hand them out.
//!
//! It needs only `core` and `alloc`. IDs cross the C ABI in a later milestone, and they are the
//! foundation for the ECS, assets, and FFI.
//!
//! # Guarantees
//!
//! - [`GenId`] is 8 bytes (`Option<GenId<T>>` too), `Copy`, and `Send + Sync`
//!   for every `T`. Its bit encoding ([`UntypedGenId::to_bits`], which
//!   [`GenId::to_bits`] forwards to) is frozen.
//! - Everything is deterministic: the IDs and iteration order a map produces
//!   are a pure function of the sequence of operations performed on it.
//!
//! # Choosing a container
//!
//! | Need | Use |
//! | --- | --- |
//! | Store values, iterate them contiguously | [`GenIdMap`] |
//! | Only IDs (data lives elsewhere), no iteration | [`GenIdAllocator`] |
//! | Only IDs, and iterate the live ones | [`GenIdMap<()>`](GenIdMap) |
//! | Work with IDs whose `T` is not known at compile time | [`UntypedGenId`] |
//!
//! Both containers issue identical IDs for the same sequence of operations.
//!
//! # Glossary
//!
//! The terms this module uses, in alphabetical order.
//!
//! | Term | Meaning |
//! | --- | --- |
//! | [`GenId<T>`](GenId) | **"GenId" is short for generational ID.** A GenId slot index plus that slot's GenId generation when the ID was issued, typed by what it names (`T`) and checked whenever it is used. It identifies a value but does not point to it or own it. |
//! | GenId generation | The counter each GenId slot keeps, bumped every time the slot is freed. A [`GenId`] resolves only while its generation equals its slot's. |
//! | GenId slot, GenId slot index | A position in the slot array of a [`GenIdMap`] or [`GenIdAllocator`]. The index is reused after the slot is freed, with a higher GenId generation, so an index alone is not unique over time. |
//! | [`GenIdAllocator<T>`](GenIdAllocator) | Hands out and frees [`GenId`]s without storing values; the data lives elsewhere, for example in a side table indexed by the GenId slot index. |
//! | [`GenIdMap<T>`](GenIdMap) | Stores values, hands out a [`GenId`] for each, and maps that ID back to its value until the value is removed. Values are kept contiguous, so iterating is a walk over one array. `GenIdMap<()>` stores no values and is the way to iterate live IDs. |
//! | `Handle<T>`, `HandlePool<T>`, `HandleAllocator<T>` | The names of [`GenId<T>`](GenId), [`GenIdMap<T>`](GenIdMap) and [`GenIdAllocator<T>`](GenIdAllocator) before ADR 0012, still used in older documents. |
//! | Retired GenId slot | A GenId slot whose generation reached its maximum. It is never reused, so a stale [`GenId`] can never resolve to a new value. |
//! | Stale [`GenId`] | A [`GenId`] whose slot has been freed (and perhaps reused) since the ID was issued. Lookups with it return `None`. |
//! | [`UntypedGenId`] | A [`GenId`] without its type, for code that does not know `T` at compile time (registries, serialisation, the C ABI). It carries the frozen bit encoding. |
//!
//! # Known limitation
//!
//! A [`GenId<T>`] issued by one map or allocator and used on a *different* one of the same
//! `T` can resolve to an unrelated element. Detecting this would need extra bits
//! per ID, breaking the 8-byte size and the FFI encoding, or branded
//! lifetimes that make the API much harder to use. Treat an ID as meaningful
//! only for the map or allocator that issued it.
//!
//! # Quick start
//!
//! ```
//! use engine::genid::GenId;
//!
//! struct Enemy;
//! let h = GenId::<Enemy>::from_bits((0 << 32) | 1).unwrap();
//! assert_eq!(GenId::<Enemy>::from_bits(h.to_bits()), Some(h));
//! ```

mod allocator;
mod error;
mod id;
mod map;
mod slots;

pub use allocator::GenIdAllocator;
#[cfg(any(test, feature = "test-invariants"))]
pub use error::InvariantViolation;
pub use error::{CapacityError, InvalidGenIdBits};
pub use id::{GenId, UntypedGenId};
pub use map::{Drain, GenIdMap, IntoIter, Iter, IterMut};
