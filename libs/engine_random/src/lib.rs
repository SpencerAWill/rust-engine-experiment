#![no_std]
#![forbid(unsafe_code)]
//! A fast, deterministic, versioned-stable pseudorandom number generator for simulation.
//!
//! `engine_random` is a separate crate because its output streams are frozen for all of
//! 0.x (a different change cadence from the rest of the engine), because it is `no_std`
//! with no allocation, and because it is useful on its own (ADR 0004).
//!
//! # Guarantees
//!
//! - **Determinism:** the same seed produces the same output on every target, including
//!   32-bit ones. Results never depend on pointer width.
//! - **Stream stability:** for a given seed, every method's output sequence is frozen for
//!   all of 0.x. Golden tests enforce this, and any change is a breaking change that
//!   requires an ADR.
//! - **Not cryptographically secure.** Do not use it for secrets or tokens.
//!
//! The generator is xoshiro256++, seeded by `SplitMix64` (see [`Prng`] for the exact
//! definitions). [`Prng`] is deliberately neither `Copy` nor `Default`; the reasons are on
//! the type.
//!
//! # Sources
//!
//! - D. Blackman and S. Vigna, "Scrambled Linear Pseudorandom Number Generators" (2018),
//!   for xoshiro256++.
//! - G. Steele, D. Lea and C. Flood, "Fast Splittable Pseudorandom Number Generators"
//!   (2014), for `SplitMix64`.
//!
//! Both are implemented from their published descriptions.
//!
//! # Quick start
//!
//! ```
//! use engine_random::Prng;
//!
//! let mut rng = Prng::from_seed(2024);
//! let roll = rng.next_u32();
//! let unit = rng.next_f64(); // in [0, 1)
//! # let _ = (roll, unit);
//! ```

mod prng;
mod sample;
mod splitmix;

pub use prng::Prng;
pub use sample::SampleRange;
