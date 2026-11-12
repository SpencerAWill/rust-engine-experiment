#![no_std]
#![forbid(unsafe_code)]
//! Simulation time primitives that make fixed-step loops correct, testable, and deterministic.
//!
//! `engine_time` is a separate crate because it is `no_std` at its core (with a default `std`
//! feature that adds `SystemClock`), because it is useful on its own, and because it depends on
//! neither math nor handles, which keeps the layering honest (ADR 0004).
//!
//! # Guarantees
//!
//! - **Exact arithmetic:** time is integer nanoseconds ([`core::time::Duration`]), so
//!   accumulation never drifts.
//! - **No ambient time:** simulation code takes a [`TimeSource`] instead of reading a clock.
//!   [`ManualClock`] is the deterministic implementation; `SystemClock` (feature `std`) is
//!   the one sanctioned wall-clock reader (ADR 0003).
//! - **Snapshot-able:** all state is plain data that can be captured and restored.
//!
//! # Quick start
//!
//! ```
//! use core::time::Duration;
//! use engine_time::{ManualClock, TimeSource};
//!
//! let mut clock = ManualClock::new();
//! clock.advance(Duration::from_millis(16));
//! assert_eq!(clock.now().as_duration(), Duration::from_millis(16));
//! ```

mod clock;
mod error;
mod tick;
mod timestamp;

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
pub use clock::SystemClock;
pub use clock::{ManualClock, TimeSource};
pub use error::NonMonotonicError;
pub use tick::Tick;
pub use timestamp::Timestamp;
