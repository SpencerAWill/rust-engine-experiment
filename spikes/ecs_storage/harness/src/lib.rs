#![forbid(unsafe_code)]
//! The harness of the ECS storage spike (M3.2): the [`Workload`] interface every storage backend
//! implements, the [`Fnv1a`] hash builder the per-entity hashes are written in, and
//! [`check_conformance`], which compares a backend's checksums with a reference backend's.
//!
//! The rules of every workload are in the M3.2 spec,
//! `docs/design/m3-ecs-storage/m3.2-spike-harness/spec.md`.

mod conformance;
mod fnv;
pub mod frag_iter;
pub mod heavy_compute;
pub mod simple_iter;

use core::num::NonZeroU32;

use engine::prelude::FixedTimestep;

pub use conformance::{Divergence, check_conformance};
pub use fnv::Fnv1a;

/// The size and seed of one workload instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// The number of entities, or for `frag_iter`, the number of entities per letter.
    pub count: usize,
    /// The seed of the `Prng` whose streams give every entity's initial values.
    pub seed: u64,
}

/// A workload's identity. Implemented by one marker type per workload module.
pub trait Spec {
    /// The workload's name, used in benchmark ids and conformance messages: `"simple_iter"`, ….
    const NAME: &'static str;
    /// The values of [`Params::count`] the benchmarks run.
    const SIZES: &'static [usize];
}

/// One backend's implementation of one workload.
pub trait Workload: Sized {
    /// The workload this implements.
    type Spec: Spec;
    /// The backend's name, used in benchmark ids and conformance messages: `"vec_aos"`, ….
    const BACKEND: &'static str;
    /// Builds the initial state. Not measured.
    fn setup(params: Params) -> Self;
    /// Advances the workload by one tick. The measured unit.
    fn step(&mut self);
    /// The order-independent checksum of the current state: the wrapping sum of the workload's
    /// per-entity hashes, plus any extra terms the workload names.
    fn checksum(&self) -> u64;
}

const TICK_HZ: NonZeroU32 = match NonZeroU32::new(60) {
    Some(hz) => hz,
    None => panic!("60 is non-zero"),
};

/// The step length every workload uses, in seconds: `FixedTimestep::from_hz(60).step()` as
/// `f32`, the value `agents_2d` steps with. The spec writes it `DT`.
#[must_use]
pub fn dt() -> f32 {
    FixedTimestep::from_hz(TICK_HZ).step().as_secs_f32()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dt_is_sixty_hz() {
        let sim = agents_2d::Sim::new(&agents_2d::SimConfig {
            agents: 0,
            ..agents_2d::SimConfig::default()
        });
        assert_eq!(dt(), sim.dt());
    }
}
