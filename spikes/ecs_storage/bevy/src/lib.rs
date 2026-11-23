#![forbid(unsafe_code)]
//! The `bevy_ecs` baseline backends of the ECS storage spike (M3.10).
//!
//! This crate is a measurement baseline on `bevy_ecs` 0.20.1 (default features off, `std` on).
//! It implements the harness's workloads on a `bevy_ecs` [`World`](bevy_ecs::world::World) so a
//! prototype's numbers can be read against a mature engine measured in the same harness. No code
//! is taken from `bevy_ecs` or any other project
//! ([ADR 0002](../../../docs/adr/0002-originality-and-dependency-licensing.md)); the crate only
//! calls its public API.
//!
//! The backend named `bevy_ecs` stores every component type in table storage. The backend named
//! `bevy_ecs_sparse` (`add_remove` and `agents` only) stores `B` and `Seeker` in `SparseSet`
//! storage, as `hybrid` does, and every other type in a table. Each workload
//! drives its `World` directly and on one thread, with no scheduler, no systems and no
//! application object, and no batch spawn or insert: structural changes go one entity at a
//! time, as in the prototypes. Every query a step runs is a `QueryState` built in `setup`. Changes decided inside a query are
//! recorded in a reused `Vec` and applied after the iteration, in recorded order. Every `step`
//! ends with `World::clear_trackers`, because without a schedule nothing else clears the
//! removal buffers that `remove` and `despawn` write.
//!
//! A bevy `World` holds an entity of its own, so a backend keeps its own count of live
//! workload entities where a step respawns up to a target, and no query consists of `Entity`
//! alone. The spec is `docs/chunks/m3.10-ecs-baselines/spec.md`.

pub mod add_remove;
pub mod agents;
pub mod entity_churn;
pub mod frag_iter;
pub mod heavy_compute;
pub mod simple_iter;
pub mod sprites;

#[cfg(test)]
mod tests;

use bevy_ecs::component::Component;

/// A harness type stored as a `bevy_ecs` component in table storage.
///
/// The orphan rule forbids implementing `Component` for the harness's types, so each is wrapped.
#[derive(Component, Clone, Copy, Debug)]
pub struct Table<T: Send + Sync + 'static>(pub T);

/// A harness type stored as a `bevy_ecs` component in `SparseSet` storage.
///
/// Inserting or removing one moves no table row, but it does move the entity to another
/// archetype, because bevy makes sparse-set components part of an archetype's identity.
#[derive(Component, Clone, Copy, Debug)]
#[component(storage = "SparseSet")]
pub struct Sparse<T: Send + Sync + 'static>(pub T);
