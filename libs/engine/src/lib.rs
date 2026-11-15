#![no_std]
#![forbid(unsafe_code)]
// A `std::` path in library code fails clippy (which runs with warnings denied) when `core` or
// `alloc` provides the same item, so a stray one fails `ci --quick` and not only `no-std`.
#![warn(clippy::std_instead_of_core, clippy::std_instead_of_alloc)]
#![cfg_attr(test, allow(clippy::std_instead_of_core, clippy::std_instead_of_alloc))]
//! Start here: the engine's library crate.
//!
//! `engine` holds all of the engine's library code as modules, and collects the everyday types in
//! [`prelude`], so a project depends on one crate and writes `use engine::prelude::*;`. A separate
//! crate is made only when ADR 0004 says so.
//!
//! # Module map
//!
//! | Module | What it is |
//! |---|---|
//! | [`genid`] | Typed generational IDs ([`GenId`](genid::GenId), with the untyped core [`UntypedGenId`](genid::UntypedGenId)), a dense [`GenIdMap`](genid::GenIdMap), and the lean [`GenIdAllocator`](genid::GenIdAllocator) |
//! | [`random`] | A deterministic, versioned-stable PRNG ([`Prng`](random::Prng)) |
//! | [`time`] | [`Tick`](time::Tick), [`FixedTimestep`](time::FixedTimestep), and [`TimeSource`](time::TimeSource) clocks |
//! | [`collections`] | Deterministic insertion-ordered [`IndexMap`](collections::IndexMap) and [`IndexSet`](collections::IndexSet) |
//! | [`math`] | glam re-exports, [`Transform`](math::Transform) and its large-world and 2D variants, coordinate conversion |
//!
//! # Features
//!
//! The crate is `no_std` with `alloc`.
//!
//! | Feature | Default | Enables |
//! |---|---|---|
//! | `std` | yes | `time::SystemClock`, and `std` as the transcendental backend of [`math::scalar`] and of glam. Without it, `libm` is the backend (ADR 0003), also on a target that has `std`. |
//! | `test-invariants` | no | `check_invariants` on [`GenIdMap`](genid::GenIdMap) and [`GenIdAllocator`](genid::GenIdAllocator), for integration tests and fuzz targets |
//!
//! # Collections
//!
//! Hashed lookups in engine code use [`collections::IndexMap`], which iterates in insertion order.
//! It is not in the prelude, so it cannot clash with the `indexmap` crate's name.
//!
//! ```
//! use engine::collections::IndexMap;
//!
//! let mut speeds = IndexMap::new();
//! speeds.insert("walk", 1.4);
//! speeds.insert("run", 4.0);
//! assert_eq!(speeds.keys().copied().collect::<Vec<_>>(), ["walk", "run"]);
//! ```
//!
//! # Guarantees
//!
//! - **Same-build determinism:** the same binary on the same platform with the same inputs gives
//!   identical results. Simulation code receives time, randomness, and input only as explicit
//!   parameters (a [`TimeSource`](time::TimeSource), a [`Prng`](random::Prng), input data), never
//!   from ambient state. See ADR 0003.
//! - **No global state:** every piece of state lives in an instance you create, so any number of
//!   engine instances can coexist in one process. See ADR 0008.
//! - **The host owns the loop:** the engine never takes over a thread it was not given. You call
//!   into it (for example [`FixedTimestep::accumulate`](time::FixedTimestep::accumulate)) from your
//!   own loop. See ADR 0007.
//! - **No `unsafe`:** the crate forbids it. See ADR 0001.
//! - **0.x:** all APIs are explicitly unstable.
//!
//! # Example
//!
//! A map of transforms, a seeded generator, and a fixed-step clock working together:
//!
//! ```
//! use engine::prelude::*;
//!
//! let mut map = GenIdMap::new();
//! let h = map.insert(Transform::IDENTITY);
//! let mut prng = Prng::from_seed(42);
//! let mut clock = ManualClock::new();
//! let mut step = FixedTimestep::from_hz(core::num::NonZeroU32::new(60).unwrap());
//! clock.advance(core::time::Duration::from_millis(20));
//! assert_eq!(step.accumulate(core::time::Duration::from_millis(20)).len(), 1);
//! assert!(map.contains(h) && prng.range(0..10) < 10);
//! ```

extern crate alloc;
// Unit tests use `std` (and its `vec!` and `format!` macros) whatever the features are.
#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(all(feature = "std", not(test)))]
extern crate std;

pub mod collections;
pub mod genid;
pub mod math;
pub mod random;
pub mod time;

/// The everyday types, for `use engine::prelude::*;`.
///
/// Only the crate root has a prelude: the modules expose a small, flat public surface instead.
pub mod prelude {
    pub use crate::genid::{GenId, GenIdAllocator, GenIdMap};
    pub use crate::math::{
        DVec2, DVec3, Mat4, Quat, Transform, Transform2d, Vec2, Vec3, Vec4, WorldTransform,
        WorldTransform2d, axis,
    };
    pub use crate::random::Prng;
    #[cfg(all(
        feature = "std",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
    pub use crate::time::SystemClock;
    pub use crate::time::{FixedTimestep, ManualClock, Tick, TimeSource, Timestamp};
}
