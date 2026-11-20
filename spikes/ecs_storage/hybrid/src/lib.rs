#![forbid(unsafe_code)]
//! The per-component hybrid backend of the ECS storage spike (M3.5).
//!
//! Each component type chooses its storage at compile time through [`Component::Storage`]:
//! [`TableStorage`] puts it in the archetype table of the entity's set of table types, as in
//! the archetype prototype (M3.4); [`SparseStorage`] puts it in a sparse set of its own, as in
//! the sparse-set prototype (M3.3), so inserting or removing it never moves the entity's row.
//! A [`World`] holds both kinds.
//!
//! The design follows the per-component storage choice, archetype graph and bundles described
//! for `bevy_ecs` and the archetype tables of `hecs` (see spec section 11 of the chunk,
//! `docs/chunks/m3.5-hybrid/spec.md`). The prototype is written from those descriptions; no
//! code is taken from `bevy_ecs`, `hecs` or any other engine.
//!
//! This is a spike: it exists to be measured against the other storage prototypes, not to be
//! published. The spec is `docs/chunks/m3.5-hybrid/spec.md`.

mod archetype;
mod bundle;
mod commands;
mod component;
pub mod frag_iter;
mod harness_components;
pub mod heavy_compute;
mod query;
pub mod simple_iter;
mod sparse;
mod table;
mod world;

#[cfg(test)]
mod tests;

use engine::genid::GenId;

pub use bundle::Bundle;
pub use commands::Commands;
pub use component::{Component, SparseStorage, Storage, TableStorage};
pub use query::{Filter, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};
pub use world::{InvariantViolation, World};

/// The marker type of [`Entity`] IDs.
#[derive(Debug)]
pub struct EntityMarker;

/// A handle to a live or despawned entity of a [`World`]: an index and a generation.
pub type Entity = GenId<EntityMarker>;
