//! Tests of the reference backends: the rules of each workload (R9), order independence (R7),
//! and that `check_conformance` detects broken candidates (R8).

use engine::prelude::*;
use spike_ecs_harness::{Params, Workload, check_conformance};

use crate::simple_iter::VecSimpleIter;

/// Storage that can be reversed in place, to test that a checksum ignores storage order.
trait Reversible {
    fn reverse(&mut self);
}

/// Spec §8 R7: reverse a clone's storage after setup and after 10 steps, then compare the
/// checksums of both over 20 further steps.
fn assert_order_independent<W: Workload + Clone + Reversible>(params: Params) {
    for steps_before in [0, 10] {
        let mut original = W::setup(params);
        for _ in 0..steps_before {
            original.step();
        }
        let mut reversed = original.clone();
        reversed.reverse();
        assert_eq!(
            original.checksum(),
            reversed.checksum(),
            "after {steps_before} steps"
        );
        for tick in 1..=20 {
            original.step();
            reversed.step();
            assert_eq!(
                original.checksum(),
                reversed.checksum(),
                "tick {tick} after {steps_before} steps"
            );
        }
    }
}

/// Review focus 1: an empty workload sets up, has checksum 0, and steps without panicking.
fn assert_zero_count_is_valid<W: Workload>() {
    let mut w = W::setup(Params { count: 0, seed: 42 });
    assert_eq!(w.checksum(), 0);
    for _ in 0..5 {
        w.step();
    }
    assert_eq!(w.checksum(), 0);
}

/// A broken candidate that applies `step` twice.
struct StepTwice<W>(W);

impl<W: Workload> Workload for StepTwice<W> {
    type Spec = W::Spec;
    const BACKEND: &'static str = "step_twice";
    fn setup(params: Params) -> Self {
        Self(W::setup(params))
    }
    fn step(&mut self) {
        self.0.step();
        self.0.step();
    }
    fn checksum(&self) -> u64 {
        self.0.checksum()
    }
}

const P: Params = Params {
    count: 1_000,
    seed: 42,
};
const TICKS: u64 = 120;

mod simple_iter {
    use super::*;
    use spike_ecs_harness::simple_iter::{self as rules, Matrix, Position, Rotation, Velocity};

    use crate::simple_iter::SimpleIterEntity;

    impl Reversible for VecSimpleIter {
        fn reverse(&mut self) {
            self.entities.reverse();
        }
    }

    /// Skips entity 0's update.
    #[derive(Clone)]
    struct SkipOneSimpleIter(VecSimpleIter);

    impl Workload for SkipOneSimpleIter {
        type Spec = <VecSimpleIter as Workload>::Spec;
        const BACKEND: &'static str = "skip_one";
        fn setup(params: Params) -> Self {
            Self(VecSimpleIter::setup(params))
        }
        fn step(&mut self) {
            for e in &mut self.0.entities[1..] {
                rules::update(&mut e.position, &e.velocity);
            }
        }
        fn checksum(&self) -> u64 {
            self.0.checksum()
        }
    }

    #[test]
    fn simple_iter_spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let mut r = prng.stream(7);
        let mut draw = |lo: f32, hi: f32| r.range(lo..hi);
        let position = Vec3::new(
            draw(-100.0, 100.0),
            draw(-100.0, 100.0),
            draw(-100.0, 100.0),
        );
        let rotation = Vec3::new(draw(-1.0, 1.0), draw(-1.0, 1.0), draw(-1.0, 1.0));
        let velocity = Vec3::new(draw(-1.0, 1.0), draw(-1.0, 1.0), draw(-1.0, 1.0));
        let (m, p, ro, v) = rules::spawn(&prng, 7);
        assert_eq!(m, Matrix(Mat4::IDENTITY));
        assert_eq!(
            (p, ro, v),
            (Position(position), Rotation(rotation), Velocity(velocity))
        );
    }

    #[test]
    fn simple_iter_moves_position_by_velocity() {
        let mut w = VecSimpleIter::setup(P);
        let before: Vec<SimpleIterEntity> = w.entities.clone();
        w.step();
        for (old, new) in before.iter().zip(&w.entities) {
            assert_eq!(new.position.0, old.position.0 + old.velocity.0);
            assert_eq!(new.matrix, old.matrix);
            assert_eq!(new.rotation, old.rotation);
            assert_eq!(new.velocity, old.velocity);
        }
    }

    #[test]
    fn simple_iter_is_order_independent() {
        assert_order_independent::<VecSimpleIter>(P);
    }

    #[test]
    fn simple_iter_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecSimpleIter>();
    }

    #[test]
    fn simple_iter_long_run_stays_finite() {
        let mut w = VecSimpleIter::setup(Params {
            count: 10,
            seed: 42,
        });
        for _ in 0..10_000 {
            w.step();
        }
        for e in &w.entities {
            assert!(e.position.0.is_finite() && e.velocity.0.is_finite());
        }
    }

    #[test]
    fn skipping_one_entity_is_detected() {
        let d = check_conformance::<VecSimpleIter, SkipOneSimpleIter>(P, TICKS).unwrap_err();
        assert_eq!(d.tick, 1);
    }

    #[test]
    fn stepping_twice_is_detected() {
        let d = check_conformance::<VecSimpleIter, StepTwice<VecSimpleIter>>(P, TICKS).unwrap_err();
        assert_eq!(d.tick, 1);
    }
}
