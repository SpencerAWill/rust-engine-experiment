//! Tests of the reference backends: the rules of each workload (R9), order independence (R7),
//! and that `check_conformance` detects broken candidates (R8).

use engine::prelude::*;
use spike_ecs_harness::{Params, Workload, check_conformance};

use crate::add_remove::VecAddRemove;
use crate::agents::{SoaAgents, VecAgents};
use crate::entity_churn::VecEntityChurn;
use crate::frag_iter::VecFragIter;
use crate::heavy_compute::VecHeavyCompute;
use crate::hierarchy::VecHierarchy;
use crate::many_queries::VecManyQueries;
use crate::random_access::VecRandomAccess;
use crate::simple_iter::VecSimpleIter;
use crate::sprites::VecSprites;
use crate::wide_entities::VecWideEntities;

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

/// Steps `w` `TICKS` times; no tick's checksum may equal an earlier one (the state keeps changing
/// through the last conformance tick). `name` labels the failure.
fn assert_checksum_never_repeats<W: Workload>(name: &str, params: Params) {
    let mut w = W::setup(params);
    let mut seen = vec![w.checksum()];
    for tick in 1..=TICKS {
        w.step();
        let sum = w.checksum();
        if let Some(earlier) = seen.iter().position(|&c| c == sum) {
            panic!("{name}: the checksum at tick {tick} equals the one at tick {earlier}");
        }
        seen.push(sum);
    }
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

mod frag_iter {
    use super::*;
    use spike_ecs_harness::frag_iter::{self as rules, Data, LETTERS, Letter};

    impl Reversible for VecFragIter {
        fn reverse(&mut self) {
            self.entities.reverse();
        }
    }

    const FRAG: Params = Params {
        count: 20,
        seed: 42,
    };

    #[test]
    fn frag_iter_spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let mut r = prng.stream(45);
        let value = r.range(0.0_f32..1.0);
        let data = r.range(0.0_f32..1.0);
        let (letter, d) = rules::spawn(&prng, 45, 20);
        // 45 / 20 = 2, which is the letter C.
        assert!(matches!(letter, Letter::C(_)));
        assert_eq!(letter.index(), 2);
        assert_eq!((letter.value(), d), (value, Data(data)));
    }

    #[test]
    #[should_panic(expected = "count must be at least 1")]
    fn frag_iter_spawn_rejects_a_zero_count() {
        let _ = rules::spawn(&Prng::from_seed(42), 0, 0);
    }

    #[test]
    #[should_panic(expected = "is not below 26 * count")]
    fn frag_iter_spawn_rejects_an_index_past_the_last_letter() {
        let _ = rules::spawn(&Prng::from_seed(42), 520, 20);
    }

    #[test]
    fn frag_iter_has_count_entities_per_letter() {
        let w = VecFragIter::setup(FRAG);
        let mut per_letter = [0_usize; LETTERS as usize];
        for (letter, _) in &w.entities {
            per_letter[usize::from(letter.index())] += 1;
        }
        assert_eq!(per_letter, [20; LETTERS as usize]);
        assert_eq!(w.entities.len(), 26 * 20);
    }

    #[test]
    fn frag_iter_data_follows_the_map() {
        let mut w = VecFragIter::setup(FRAG);
        // One input that wraps (0.5 + 0.618_034 >= 1.0) and one that does not.
        w.entities[0].1 = Data(0.5);
        w.entities[1].1 = Data(0.25);
        let before: Vec<f32> = w.entities.iter().map(|(_, d)| d.0).collect();
        w.step();
        assert_eq!(w.entities[0].1.0, 0.5_f32 + 0.618_034 - 1.0);
        assert_eq!(w.entities[1].1.0, 0.25_f32 + 0.618_034);
        for (old, (_, new)) in before.iter().zip(&w.entities) {
            let x = old + 0.618_034;
            assert_eq!(new.0, if x >= 1.0 { x - 1.0 } else { x });
        }
    }

    #[test]
    fn frag_iter_checksum_never_repeats() {
        assert_checksum_never_repeats::<VecFragIter>("frag_iter", FRAG);
    }

    #[test]
    fn frag_iter_is_order_independent() {
        assert_order_independent::<VecFragIter>(FRAG);
    }

    #[test]
    fn frag_iter_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecFragIter>();
    }

    #[test]
    fn frag_iter_long_run_stays_finite() {
        let mut w = VecFragIter::setup(Params {
            count: 10,
            seed: 42,
        });
        for _ in 0..10_000 {
            w.step();
        }
        for (letter, data) in &w.entities {
            assert!(letter.value().is_finite() && data.0.is_finite());
        }
    }
}

mod heavy_compute {
    use super::*;
    use spike_ecs_harness::heavy_compute::{self as rules, INVERSIONS};

    impl Reversible for VecHeavyCompute {
        fn reverse(&mut self) {
            self.entities.reverse();
        }
    }

    const HEAVY: Params = Params {
        count: 1_000,
        seed: 42,
    };

    #[test]
    fn heavy_compute_spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let mut r = prng.stream(3);
        let mut vec3 =
            |lo: f32, hi: f32| Vec3::new(r.range(lo..hi), r.range(lo..hi), r.range(lo..hi));
        let position = vec3(-100.0, 100.0);
        let rotation = vec3(-1.0, 1.0);
        let velocity = vec3(-1.0, 1.0);
        let mut cols = [Vec4::ZERO; 4];
        for (c, col) in cols.iter_mut().take(3).enumerate() {
            for row in 0..3 {
                col[row] = if row == c {
                    r.range(2.0_f32..4.0)
                } else {
                    r.range(-0.5_f32..0.5)
                };
            }
        }
        for row in 0..3 {
            cols[3][row] = r.range(-10.0_f32..10.0);
        }
        cols[3][3] = 1.0;
        let (m, p, ro, v) = rules::spawn(&prng, 3);
        assert_eq!(m.0, Mat4::from_cols(cols[0], cols[1], cols[2], cols[3]));
        assert_eq!(m.0.row(3), Vec4::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!((p.0, ro.0, v.0), (position, rotation, velocity));
    }

    #[test]
    fn heavy_compute_matrices_are_invertible() {
        let w = VecHeavyCompute::setup(HEAVY);
        assert_eq!(w.entities.len(), 1_000);
        for e in &w.entities {
            assert_ne!(e.matrix.0.determinant(), 0.0);
        }
    }

    #[test]
    fn heavy_compute_position_is_the_transformed_rotation() {
        let mut w = VecHeavyCompute::setup(HEAVY);
        let before = w.entities.clone();
        w.step();
        assert_eq!(INVERSIONS, 100);
        let dt = spike_ecs_harness::dt();
        for (old, e) in before.iter().zip(&w.entities) {
            // The rule written out by hand: invert 100 times, move the translation by
            // `velocity * dt` (multiply, then add), then transform the rotation.
            let mut m = old.matrix.0;
            for _ in 0..100 {
                m = m.inverse();
            }
            for r in 0..3 {
                let moved = old.velocity.0[r] * dt;
                m.w_axis[r] += moved;
            }
            assert_eq!(e.matrix.0, m);
            assert_eq!(e.position.0, m.transform_point3(e.rotation.0));
        }
    }

    #[test]
    fn heavy_compute_checksum_never_repeats() {
        assert_checksum_never_repeats::<VecHeavyCompute>("heavy_compute", HEAVY);
    }

    #[test]
    fn heavy_compute_is_order_independent() {
        assert_order_independent::<VecHeavyCompute>(HEAVY);
    }

    #[test]
    fn heavy_compute_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecHeavyCompute>();
    }

    #[test]
    fn heavy_compute_long_run_stays_finite() {
        let mut w = VecHeavyCompute::setup(Params {
            count: 10,
            seed: 42,
        });
        for _ in 0..10_000 {
            w.step();
        }
        for e in &w.entities {
            assert!(e.matrix.0.is_finite() && e.position.0.is_finite());
            assert_ne!(e.matrix.0.determinant(), 0.0);
        }
    }
}

mod add_remove {
    use super::*;
    use spike_ecs_harness::add_remove::{self as rules, A};

    impl Reversible for VecAddRemove {
        fn reverse(&mut self) {
            self.entities.reverse();
        }
    }

    /// Leaves entity 0's `B` in place when removing.
    struct SkipOneRemoval(VecAddRemove);

    impl Workload for SkipOneRemoval {
        type Spec = <VecAddRemove as Workload>::Spec;
        const BACKEND: &'static str = "skip_one_removal";
        fn setup(params: Params) -> Self {
            Self(VecAddRemove::setup(params))
        }
        fn step(&mut self) {
            for (a, b) in &mut self.0.entities {
                *b = Some(rules::make_b(a));
            }
            for (_, b) in &mut self.0.entities[1..] {
                *b = None;
            }
        }
        fn checksum(&self) -> u64 {
            self.0.checksum()
        }
    }

    #[test]
    fn add_remove_spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let expected = prng.stream(9).range(0.0_f32..1.0);
        assert_eq!(rules::spawn(&prng, 9), A(expected));
    }

    #[test]
    fn add_remove_leaves_no_b_after_a_step() {
        let mut w = VecAddRemove::setup(P);
        assert!(w.entities.iter().all(|(_, b)| b.is_none()));
        for _ in 0..3 {
            w.step();
            assert!(w.entities.iter().all(|(_, b)| b.is_none()));
        }
    }

    #[test]
    fn add_remove_vec_inserts_on_every_entity_each_step() {
        let mut w = VecAddRemove::setup(P);
        for tick in 1..=TICKS {
            let (mut calls, mut seen) = (0_u64, 0_usize);
            w.step_with(|entities| {
                calls += 1;
                seen += entities.len();
                for (i, (a, b)) in entities.iter().enumerate() {
                    assert_eq!(
                        *b,
                        Some(rules::make_b(a)),
                        "add_remove: entity {i} holds the wrong B between the passes at tick {tick}"
                    );
                }
            });
            assert_eq!(
                calls, 1,
                "add_remove: between ran {calls} times at tick {tick}"
            );
            assert_eq!(
                seen, P.count,
                "add_remove: between saw {seen} entities at tick {tick}"
            );
        }
    }

    #[test]
    fn add_remove_is_order_independent() {
        assert_order_independent::<VecAddRemove>(P);
    }

    #[test]
    fn add_remove_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecAddRemove>();
    }

    #[test]
    fn skipping_one_removal_is_detected() {
        let d = check_conformance::<VecAddRemove, SkipOneRemoval>(P, TICKS).unwrap_err();
        assert_eq!(d.tick, 1);
    }
}

mod entity_churn {
    use super::*;
    use spike_ecs_harness::entity_churn::{self as rules, Lifetime, Position, SpawnId, Velocity};

    impl Reversible for VecEntityChurn {
        fn reverse(&mut self) {
            self.entities.reverse();
        }
    }

    /// Skips entity 0's update.
    struct SkipOneChurn(VecEntityChurn);

    impl Workload for SkipOneChurn {
        type Spec = <VecEntityChurn as Workload>::Spec;
        const BACKEND: &'static str = "skip_one";
        fn setup(params: Params) -> Self {
            Self(VecEntityChurn::setup(params))
        }
        fn step(&mut self) {
            for e in &mut self.0.entities[1..] {
                rules::update(&mut e.position, &e.velocity, &mut e.lifetime);
            }
            self.0.replace_expired();
        }
        fn checksum(&self) -> u64 {
            self.0.checksum()
        }
    }

    #[test]
    fn entity_churn_spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let mut r = prng.stream(11);
        let position = Vec2::new(r.range(0.0_f32..320.0), r.range(0.0_f32..180.0));
        let velocity = Vec2::new(r.range(-1.0_f32..1.0), r.range(-1.0_f32..1.0));
        let lifetime = r.range(1_u32..=30);
        assert_eq!(
            rules::spawn(&prng, 11),
            (
                SpawnId(11),
                Position(position),
                Velocity(velocity),
                Lifetime(lifetime)
            )
        );
    }

    #[test]
    fn entity_churn_keeps_count_alive() {
        let mut w = VecEntityChurn::setup(P);
        for _ in 0..TICKS {
            w.step();
            assert_eq!(w.entities.len(), P.count);
            assert!(w.entities.iter().all(|e| e.lifetime.0 > 0));
            let mut ids: Vec<u64> = w.entities.iter().map(|e| e.id.0).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), P.count, "spawn ids are unique");
        }
    }

    #[test]
    fn entity_churn_new_ids_increase() {
        let mut w = VecEntityChurn::setup(P);
        for _ in 0..TICKS {
            let before = w.next_id;
            w.step();
            let mut fresh: Vec<u64> = w
                .entities
                .iter()
                .map(|e| e.id.0)
                .filter(|&id| id >= before)
                .collect();
            fresh.sort_unstable();
            assert_eq!(fresh, (before..w.next_id).collect::<Vec<_>>());
        }
    }

    #[test]
    fn entity_churn_replaces_some_entities() {
        let mut w = VecEntityChurn::setup(P);
        for _ in 0..TICKS {
            w.step();
        }
        let replaced = w.next_id - P.count as u64;
        #[allow(clippy::cast_precision_loss, reason = "small counts")]
        let per_step = replaced as f64 / TICKS as f64 / P.count as f64;
        assert!((0.04..0.09).contains(&per_step), "{per_step}");
    }

    #[test]
    fn entity_churn_is_order_independent() {
        assert_order_independent::<VecEntityChurn>(P);
    }

    #[test]
    fn entity_churn_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecEntityChurn>();
    }

    #[test]
    fn single_entity_churns_forever() {
        let mut w = VecEntityChurn::setup(Params { count: 1, seed: 42 });
        let mut last = w.entities[0].id.0;
        for _ in 0..200 {
            w.step();
            assert_eq!(w.entities.len(), 1);
            let id = w.entities[0].id.0;
            assert!(id >= last);
            last = id;
        }
        assert!(last > 5, "the entity was replaced repeatedly: {last}");
    }

    #[test]
    fn skipping_one_churn_entity_is_detected() {
        let d = check_conformance::<VecEntityChurn, SkipOneChurn>(P, TICKS).unwrap_err();
        assert_eq!(d.tick, 1);
    }

    #[test]
    fn stepping_churn_twice_is_detected() {
        let d =
            check_conformance::<VecEntityChurn, StepTwice<VecEntityChurn>>(P, TICKS).unwrap_err();
        assert_eq!(d.tick, 1);
    }
}

mod sprites {
    use super::*;
    use agents_2d::{WORLD_HEIGHT, WORLD_WIDTH};
    use spike_ecs_harness::sprites::{self as rules, DrawItem, Velocity2d};

    const SPRITES: Params = Params {
        count: 1_000,
        seed: 42,
    };

    impl Reversible for VecSprites {
        fn reverse(&mut self) {
            self.entities.reverse();
            self.draw_list.reverse();
        }
    }

    #[test]
    fn sprites_spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let mut r = prng.stream(5);
        let translation = Vec2::new(r.range(0.0_f32..320.0), r.range(0.0_f32..180.0));
        let velocity = Vec2::new(r.range(-20.0_f32..20.0), r.range(-20.0_f32..20.0));
        let color = r.next_u32() & 0x00FF_FFFF;
        let half_extent = r.range(0.25_f32..2.0);
        let hidden = r.chance(0.1);
        let (t, v, sprite, tag) = rules::spawn(&prng, 5);
        assert_eq!(t, Transform2d::IDENTITY.with_translation(translation));
        assert_eq!(v, Velocity2d(velocity));
        assert_eq!((sprite.color, sprite.half_extent), (color, half_extent));
        assert_eq!(tag.is_some(), hidden);
    }

    #[test]
    fn sprites_stay_inside_the_world() {
        let mut w = VecSprites::setup(SPRITES);
        for _ in 0..TICKS {
            w.step();
            for e in &w.entities {
                let p = e.transform.translation;
                // Closed range: a tiny negative coordinate can round onto the bound.
                assert!((0.0..=WORLD_WIDTH).contains(&p.x), "{p}");
                assert!((0.0..=WORLD_HEIGHT).contains(&p.y), "{p}");
            }
        }
    }

    #[test]
    fn wrap_of_a_tiny_negative_lands_on_the_bound() {
        assert_eq!(rules::wrap(-1e-6, 320.0), 320.0);
        assert_eq!(rules::wrap(320.0, 320.0), 0.0);
        assert_eq!(rules::wrap(-1.0, 320.0), 319.0);
        assert_eq!(rules::wrap(5.0, 320.0), 5.0);
    }

    #[test]
    fn sprites_draw_list_holds_exactly_the_visible() {
        let mut w = VecSprites::setup(SPRITES);
        assert!(w.draw_list.is_empty(), "setup leaves the draw list empty");
        w.step();
        let mut expected: Vec<DrawItem> = w
            .entities
            .iter()
            .filter(|e| e.hidden.is_none())
            .map(|e| rules::draw_item(&e.transform, &e.sprite))
            .collect();
        let hidden = w.entities.iter().filter(|e| e.hidden.is_some()).count();
        assert!(hidden > 0, "the sample has hidden sprites");
        assert_eq!(w.draw_list.len(), SPRITES.count - hidden);
        let mut actual = w.draw_list.clone();
        expected.sort_by_key(rules::hash_draw_item);
        actual.sort_by_key(rules::hash_draw_item);
        assert_eq!(actual, expected);
    }

    #[test]
    fn sprites_hidden_sprites_still_move() {
        let mut w = VecSprites::setup(SPRITES);
        let before: Vec<Vec2> = w.entities.iter().map(|e| e.transform.translation).collect();
        w.step();
        let mut moved = 0;
        for (old, e) in before.iter().zip(&w.entities) {
            if e.hidden.is_some() {
                assert_ne!(e.transform.translation, *old);
                moved += 1;
            }
        }
        assert!(moved > 0);
    }

    #[test]
    fn sprites_is_order_independent() {
        assert_order_independent::<VecSprites>(SPRITES);
    }

    #[test]
    fn sprites_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecSprites>();
        let mut w = VecSprites::setup(Params { count: 0, seed: 42 });
        w.step();
        assert_eq!(w.draw_list.len(), 0);
    }

    #[test]
    fn sprites_long_run_stays_finite() {
        let mut w = VecSprites::setup(Params {
            count: 10,
            seed: 42,
        });
        for _ in 0..10_000 {
            w.step();
        }
        for e in &w.entities {
            assert!(e.transform.translation.is_finite());
        }
    }
}

mod agents {
    use super::*;
    use agents_2d::SimConfig;

    impl Reversible for SoaAgents {
        fn reverse(&mut self) {
            self.ids.reverse();
            self.transforms.reverse();
            self.velocities.reverse();
            self.ttls.reverse();
            self.seekers.reverse();
        }
    }

    const AGENTS: Params = Params {
        count: 1_000,
        seed: 42,
    };

    #[test]
    fn vec_aos_agents_is_sim() {
        let mut w = VecAgents::setup(AGENTS);
        for _ in 0..600 {
            w.step();
        }
        let config = SimConfig {
            agents: 1_000,
            seed: 42,
        };
        assert_eq!(w.checksum(), agents_2d::run(&config, 600).checksum);
    }

    #[test]
    fn vec_soa_agents_matches_vec_aos() {
        // 1,300 ticks cover respawns (lifetimes end by tick 600) and a period without a cursor.
        assert_eq!(
            check_conformance::<VecAgents, SoaAgents>(AGENTS, 1_300),
            Ok(())
        );
    }

    #[test]
    fn soa_agents_is_order_independent() {
        assert_order_independent::<SoaAgents>(AGENTS);
    }

    #[test]
    fn agents_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecAgents>();
        assert_zero_count_is_valid::<SoaAgents>();
    }
}

#[test]
fn random_access_zero_count_is_valid() {
    assert_zero_count_is_valid::<VecRandomAccess>();
}

/// R4: within the conformance run, some entities hold a stale handle and are retargeted, so the
/// conformance checksums cover the stale path.
#[test]
#[allow(
    clippy::print_stdout,
    reason = "the plan asks for the count to be printed"
)]
fn random_access_retargets_within_conformance_ticks() {
    let mut w = VecRandomAccess::setup(Params {
        count: 1_000,
        seed: 42,
    });
    let mut retargets = 0_usize;
    for _ in 0..700 {
        let before: Vec<_> = w.entities.iter().map(|e| (e.id, e.target)).collect();
        w.step();
        for (e, (id, target)) in w.entities.iter().zip(before) {
            if e.id == id && e.target.slot == target.slot && e.target.entity != target.entity {
                retargets += 1;
            }
        }
    }
    println!("random_access retargets over 700 ticks at count 1000: {retargets}");
    assert!(retargets > 0);
}

#[test]
fn random_access_respawns_fill_slots_in_order() {
    let mut w = VecRandomAccess::setup(Params {
        count: 1_000,
        seed: 42,
    });
    for _ in 0..700 {
        let first_new = w.next_id;
        w.step();
        let fresh: Vec<u64> = w
            .entities
            .iter()
            .map(|e| e.id.0)
            .filter(|&id| id >= first_new)
            .collect();
        if !fresh.is_empty() {
            // `entities` is indexed by slot, so `fresh` is in slot order.
            assert!(fresh.windows(2).all(|pair| pair[0] + 1 == pair[1]));
            assert_eq!(fresh[0], first_new);
            return;
        }
    }
    panic!("no entity expired in 700 ticks");
}

#[test]
fn many_queries_vec_zero_count_is_valid() {
    assert_zero_count_is_valid::<VecManyQueries>();
}

/// Spec §4.2: a query visits the entities that have both its types, which depends only on the
/// distance `d = (read - write) mod 8`: 375, 250, 125 or 0 of 1,000 for `d` in {1,7}, {2,6},
/// {3,5}, {4}.
#[test]
fn many_queries_match_counts_follow_the_distance() {
    let mut w = VecManyQueries::setup(Params {
        count: 1_000,
        seed: 42,
    });
    let mut counts = [0_usize; 56];
    w.step_counting(&mut counts);
    for (q, &(write, read)) in spike_ecs_harness::many_queries::QUERIES.iter().enumerate() {
        let d = (usize::from(read) + 8 - usize::from(write)) % 8;
        let expected = match d {
            1 | 7 => 375,
            2 | 6 => 250,
            3 | 5 => 125,
            4 => 0,
            _ => unreachable!("write and read differ"),
        };
        assert_eq!(counts[q], expected, "query {q} ({write}, {read})");
    }
}

/// Spec §4.2: values wrap, so the state keeps changing through the last conformance tick.
#[test]
fn many_queries_still_changes_at_the_last_tick() {
    let mut w = VecManyQueries::setup(Params {
        count: 1_000,
        seed: 42,
    });
    let mut previous = w.checksum();
    for tick in 1..=120 {
        w.step();
        let now = w.checksum();
        assert_ne!(now, previous, "tick {tick}");
        previous = now;
    }
}

mod wide_entities {
    use super::*;
    use spike_ecs_harness::wide_entities::make_extra;

    #[test]
    fn wide_entities_vec_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecWideEntities>();
    }

    #[test]
    fn wide_entities_checksum_never_repeats() {
        assert_checksum_never_repeats::<VecWideEntities>("wide_entities", P);
    }

    #[test]
    fn wide_entities_vec_inserts_between_the_passes() {
        let mut w = VecWideEntities::setup(P);
        for tick in 1..=TICKS {
            let (mut calls, mut seen) = (0_u64, 0_usize);
            w.step_with(|rows| {
                calls += 1;
                seen += rows.len();
                for (i, (wide, extra)) in rows.iter().enumerate() {
                    assert_eq!(
                        *extra,
                        Some(make_extra(&wide.w0)),
                        "wide_entities: entity {i} holds the wrong Extra between the passes at tick {tick}"
                    );
                }
            });
            assert_eq!(
                calls, 1,
                "wide_entities: between ran {calls} times at tick {tick}"
            );
            assert_eq!(
                seen, P.count,
                "wide_entities: between saw {seen} entities at tick {tick}"
            );
        }
    }
}

mod hierarchy {
    use super::*;

    #[test]
    fn hierarchy_vec_zero_count_is_valid() {
        assert_zero_count_is_valid::<VecHierarchy>();
    }

    /// Spec §4.5 (ecs-13): every tick's checksum differs from every earlier tick's, tick 0
    /// included, and every row's `GlobalTransform` differs from its value at the previous tick.
    #[test]
    fn hierarchy_checksum_never_repeats_and_every_global_changes() {
        assert_checksum_never_repeats::<VecHierarchy>("hierarchy", P);

        let mut w = VecHierarchy::setup(P);
        let mut previous = w.globals();
        assert_eq!(previous.len(), P.count);
        for tick in 1..=TICKS {
            w.step();
            let now = w.globals();
            for (i, (before, after)) in previous.iter().zip(&now).enumerate() {
                assert_ne!(
                    before, after,
                    "hierarchy: row {i} did not change at tick {tick}"
                );
            }
            previous = now;
        }
    }
}
