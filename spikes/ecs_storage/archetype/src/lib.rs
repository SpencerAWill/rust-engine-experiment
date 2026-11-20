#![forbid(unsafe_code)]
//! The archetype backend of the ECS storage spike (M3.4).
//!
//! A [`World`] keeps one table per distinct set of component types: the entities that have
//! exactly those types, and one column of values per type. Inserting or removing a component
//! moves the entity's row to the table for its new set, through transition edges cached on each
//! archetype, so a repeated transition costs an array read plus the move.
//!
//! The design follows the archetype tables and archetype graph described for `bevy_ecs` and
//! the archetype tables of `hecs` (see spec section 11 of the chunk,
//! `docs/chunks/m3.4-archetype/spec.md`). The prototype is written from those descriptions; no
//! code is taken from `bevy_ecs`, `hecs` or any other engine.
//!
//! This is a spike: it exists to be measured against the plain-`Vec` and sparse-set backends,
//! not to be published. The spec is `docs/chunks/m3.4-archetype/spec.md`.

mod archetype;
mod bundle;
mod query;
mod table;
mod world;

#[cfg(test)]
mod tests;

use engine::genid::GenId;

pub use bundle::Bundle;
pub use query::{Filter, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};
pub use world::{InvariantViolation, World};

/// The marker type of [`Entity`] IDs.
#[derive(Debug)]
pub struct EntityMarker;

/// A handle to a live or despawned entity of a [`World`]: an index and a generation.
pub type Entity = GenId<EntityMarker>;
