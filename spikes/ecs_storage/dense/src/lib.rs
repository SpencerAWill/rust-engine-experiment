#![forbid(unsafe_code)]
//! The dense-array backend of the ECS storage spike (M3.19).
//!
//! A [`World`] keeps, for each component type, one array indexed by entity index (a slot that
//! is `Some` when the entity has the component) and a hierarchical presence bit set over the
//! same indices. Queries join the presence sets of the types they require and fetch each
//! parameter at the indices the join yields, in ascending order.
//!
//! The design follows the dense `VecStorage` and the bit-set joins of the `specs` crate (see
//! spec section 11 of the chunk, `docs/chunks/m3.19-dense-arrays/spec.md`). The prototype is
//! written from those descriptions; no code is taken from `specs` or any other project.
//!
//! This is a spike: it exists to be measured against the other storage backends, not to be
//! published. The spec is `docs/chunks/m3.19-dense-arrays/spec.md`.

pub mod add_remove;
pub mod agents;
mod column;
mod commands;
pub mod entity_churn;
pub mod frag_iter;
pub mod heavy_compute;
mod lookup;
mod query;
pub mod random_access;
pub mod simple_iter;
pub mod sprites;
#[cfg(test)]
mod tests;
mod world;

use engine::genid::GenId;

pub use commands::Commands;
pub use lookup::Lookup;
pub use query::{Filter, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};
pub use world::{InvariantViolation, World};

/// The marker type of [`Entity`] IDs.
#[derive(Debug)]
pub struct EntityMarker;

/// A handle to a live or despawned entity of a [`World`]: an index and a generation.
pub type Entity = GenId<EntityMarker>;
