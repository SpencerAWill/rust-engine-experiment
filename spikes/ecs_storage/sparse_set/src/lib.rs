#![forbid(unsafe_code)]
//! The sparse-set backends of the ECS storage spike (M3.3).
//!
//! A [`World`] keeps one sparse set per component type: a packed array of values, a parallel
//! packed array of the entities that own them, and a sparse array that maps an entity's index
//! to its position in the packed arrays. Adding, removing and looking up a component is a
//! constant-time array access, and removal swaps the last entry into the hole.
//!
//! The design follows the sparse sets and owning groups described by Michele Caini (skypjack)
//! for `EnTT` (see spec section 11 of the chunk, `docs/chunks/m3.3-sparse-set/spec.md`). The
//! prototype is written from those descriptions; no code is taken from `EnTT` or any other
//! engine.
//!
//! This is a spike: it exists to be measured against the plain-`Vec` backends, not to be
//! published. The spec is `docs/chunks/m3.3-sparse-set/spec.md`.

pub mod add_remove;
pub mod agents;
mod commands;
pub mod entity_churn;
pub mod frag_iter;
pub mod heavy_compute;
pub mod hierarchy;
mod lookup;
pub mod many_queries;
mod query;
pub mod random_access;
pub mod rare_components;
pub mod simple_iter;
pub mod sprites;
mod storage;
#[cfg(test)]
mod tests;
pub mod wide_entities;
mod world;

use engine::genid::GenId;

pub use commands::Commands;
pub use lookup::Lookup;
pub use query::{
    Filter, GroupSpec, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without,
};
pub use world::{InvariantViolation, World, WorldBuilder};

/// The marker type of [`Entity`] IDs.
#[derive(Debug)]
pub struct EntityMarker;

/// A handle to a live or despawned entity of a [`World`]: an index and a generation.
pub type Entity = GenId<EntityMarker>;
