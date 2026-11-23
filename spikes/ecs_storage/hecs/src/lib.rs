#![forbid(unsafe_code)]
//! The `hecs` baseline backend of the ECS storage spike (M3.10).
//!
//! This crate is a measurement baseline on `hecs` 0.11.2 (default features off, `std` on). It
//! implements the harness's seven workloads on a `hecs` [`World`](hecs::World) so a prototype's
//! numbers can be read against a mature engine measured in the same harness. No code is taken
//! from `hecs` or any other project
//! (`docs/adr/0002-originality-and-dependency-licensing.md`); the crate only calls its public
//! API.
//!
//! Every `Send + Sync + 'static` type is a `hecs` component, so the harness's types are stored
//! unwrapped; `agents` uses its own newtypes. Each workload drives its `World` directly and on
//! one thread, with no scheduler and no batch spawn. Every step runs plain `query_mut` or
//! `query` calls: the backend uses no prepared query, which is `hecs`'s documented path for a
//! repeated query. With `std`, `hecs` caches each query type's archetype matches in the
//! `World`, so a steady-state run pays, per query, a lock, a type lookup, a downcast and an
//! `Arc` clone that the prototypes do not; read the `hecs` rows with that in mind.
//!
//! Changes decided inside a query are collected into a reused `Vec` and applied through
//! `World` (`insert_one`, `remove_one`, `despawn`) after the iteration, in recorded order. The
//! `hecs` command buffer is not used: in `hecs` 0.11.2 a buffered insert is a keyless dynamic
//! bundle, which skips the cached insert edges that `World::insert_one` follows.
//!
//! The spec is `docs/chunks/m3.10-ecs-baselines/spec.md`.

pub mod add_remove;
pub mod agents;
pub mod entity_churn;
pub mod frag_iter;
pub mod heavy_compute;
pub mod simple_iter;
pub mod sprites;

#[cfg(test)]
mod tests;
