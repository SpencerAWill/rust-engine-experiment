#![forbid(unsafe_code)]
//! Start here: the single dependency for using the engine.
//!
//! `engine` is a facade. It re-exports the foundation crates as modules and collects the
//! everyday types in [`prelude`], so a project depends on one crate and writes
//! `use engine::prelude::*;`.
//!
//! # Crate map
//!
//! | Module | Crate | What it is |
//! |---|---|---|
//! | [`handle`] | `engine_handle` | Typed generational [`Handle`](handle::Handle)s, a dense [`HandlePool`](handle::HandlePool), and two allocators: the lean [`HandleAllocator`](handle::HandleAllocator) and the iterable [`DenseHandleAllocator`](handle::DenseHandleAllocator) |
//! | [`random`] | `engine_random` | A deterministic, versioned-stable PRNG ([`Prng`](random::Prng)) |
//! | [`time`] | `engine_time` | [`Tick`](time::Tick), [`FixedTimestep`](time::FixedTimestep), and [`TimeSource`](time::TimeSource) clocks |
//! | [`math`] | `engine_math` | glam re-exports, [`Transform`](math::Transform) and its large-world and 2D variants, coordinate conversion |
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
//! - **No `unsafe`:** every crate forbids it. See ADR 0001.
//! - **0.x:** all APIs are explicitly unstable.
//!
//! # Example
//!
//! A pool of transforms, a seeded generator, and a fixed-step clock working together:
//!
//! ```
//! use engine::prelude::*;
//!
//! let mut pool = HandlePool::new();
//! let h = pool.insert(Transform::IDENTITY);
//! let mut prng = Prng::from_seed(42);
//! let mut clock = ManualClock::new();
//! let mut step = FixedTimestep::from_hz(core::num::NonZeroU32::new(60).unwrap());
//! clock.advance(core::time::Duration::from_millis(20));
//! assert_eq!(step.accumulate(core::time::Duration::from_millis(20)).len(), 1);
//! assert!(pool.contains(h) && prng.range(0..10) < 10);
//! ```

pub use engine_handle as handle;
pub use engine_math as math;
pub use engine_random as random;
pub use engine_time as time;

/// The everyday types, for `use engine::prelude::*;`.
///
/// Only this facade has a prelude: the leaf crates expose a small, flat public surface instead.
pub mod prelude {
    pub use engine_handle::{DenseHandleAllocator, Handle, HandleAllocator, HandlePool};
    pub use engine_math::{
        DVec2, DVec3, Mat4, Quat, Transform, Transform2d, Vec2, Vec3, Vec4, WorldTransform,
        WorldTransform2d, axis,
    };
    pub use engine_random::Prng;
    #[cfg(all(
        feature = "std",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
    pub use engine_time::SystemClock;
    pub use engine_time::{FixedTimestep, ManualClock, Tick, TimeSource, Timestamp};
}
