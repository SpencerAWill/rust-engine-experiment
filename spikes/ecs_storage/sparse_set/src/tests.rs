//! The model test: a `World` against a plain map, under random operations.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use crate::{Entity, World, WorldBuilder};

#[derive(Debug, Clone, PartialEq)]
struct C0(u32);
#[derive(Debug, Clone, PartialEq)]
struct C1(u32);
#[derive(Debug, Clone, PartialEq)]
struct C2(u32);
#[derive(Debug, Clone, PartialEq)]
struct C3(u32);

#[derive(Debug, Clone)]
enum Op {
    Spawn,
    Despawn(usize),
    Insert(usize, usize, u32),
    Remove(usize, usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::Spawn),
        1 => any::<usize>().prop_map(Op::Despawn),
        5 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::Insert(k, t, v)),
        3 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::Remove(k, t)),
    ]
}

fn insert(w: &mut World, e: Entity, t: usize, v: u32) -> Option<u32> {
    match t {
        0 => w.insert(e, C0(v)).map(|c| c.0),
        1 => w.insert(e, C1(v)).map(|c| c.0),
        2 => w.insert(e, C2(v)).map(|c| c.0),
        _ => w.insert(e, C3(v)).map(|c| c.0),
    }
}

fn remove(w: &mut World, e: Entity, t: usize) -> Option<u32> {
    match t {
        0 => w.remove::<C0>(e).map(|c| c.0),
        1 => w.remove::<C1>(e).map(|c| c.0),
        2 => w.remove::<C2>(e).map(|c| c.0),
        _ => w.remove::<C3>(e).map(|c| c.0),
    }
}

fn get(w: &World, e: Entity, t: usize) -> Option<u32> {
    match t {
        0 => w.get::<C0>(e).map(|c| c.0),
        1 => w.get::<C1>(e).map(|c| c.0),
        2 => w.get::<C2>(e).map(|c| c.0),
        _ => w.get::<C3>(e).map(|c| c.0),
    }
}

fn build(setup: usize) -> World {
    let b = WorldBuilder::new();
    match setup {
        0 => b.build(),
        1 => b.group::<(C0, C1)>().build(),
        _ => b.group::<(C0, C1, C2)>().build(),
    }
}

fn bits(ids: impl Iterator<Item = Entity>) -> Vec<u64> {
    let mut v: Vec<u64> = ids.map(Entity::to_bits).collect();
    v.sort_unstable();
    v
}

/// The entities the model says match, as sorted bits.
fn expected(
    model: &BTreeMap<u64, [Option<u32>; 4]>,
    want: impl Fn(&[Option<u32>; 4]) -> bool,
) -> Vec<u64> {
    model
        .iter()
        .filter(|(_, c)| want(c))
        .map(|(k, _)| *k)
        .collect()
}

fn check_queries(w: &World, model: &BTreeMap<u64, [Option<u32>; 4]>) {
    let got = bits(w.query::<(Entity, &C0, &C1)>().iter().map(|(e, _, _)| e));
    assert_eq!(got, expected(model, |c| c[0].is_some() && c[1].is_some()));
    let got = bits(
        w.query::<(Entity, &C0)>()
            .without::<C2>()
            .iter()
            .map(|(e, _)| e),
    );
    assert_eq!(got, expected(model, |c| c[0].is_some() && c[2].is_none()));
    let got = bits(
        w.query::<(Entity, Option<&C3>)>()
            .with::<C1>()
            .iter()
            .map(|(e, _)| e),
    );
    assert_eq!(got, expected(model, |c| c[1].is_some()));
}

#[allow(clippy::many_single_char_names)] // model-test locals: w, e, t, v, k
fn run(setup: usize, ops: &[Op]) {
    let mut w = build(setup);
    let mut spawned: Vec<Entity> = Vec::new();
    let mut model: BTreeMap<u64, [Option<u32>; 4]> = BTreeMap::new();
    let mut live: BTreeSet<u64> = BTreeSet::new();
    for op in ops {
        match *op {
            Op::Spawn => {
                let e = w.spawn();
                spawned.push(e);
                model.insert(e.to_bits(), [None; 4]);
                live.insert(e.to_bits());
            }
            Op::Despawn(k) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                let was_live = live.remove(&e.to_bits());
                assert_eq!(w.despawn(e), was_live);
                if was_live {
                    model.insert(e.to_bits(), [None; 4]);
                }
            }
            Op::Insert(k, t, v) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                if live.contains(&e.to_bits()) {
                    let old = model.get_mut(&e.to_bits()).unwrap()[t].replace(v);
                    assert_eq!(insert(&mut w, e, t, v), old);
                }
            }
            Op::Remove(k, t) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                let old = if live.contains(&e.to_bits()) {
                    model.get_mut(&e.to_bits()).unwrap()[t].take()
                } else {
                    None
                };
                assert_eq!(remove(&mut w, e, t), old);
            }
            _ => {}
        }
        assert_eq!(w.check_invariants(), Ok(()));
        assert_eq!(w.len(), live.len());
        check_queries(&w, &model);
        for e in &spawned {
            for (t, expected) in model[&e.to_bits()].iter().enumerate() {
                assert_eq!(get(&w, *e, t), *expected, "{e:?} type {t}");
            }
        }
    }
}

proptest! {
    #[test]
    fn world_matches_the_model(ops in prop::collection::vec(op(), 0..64)) {
        for setup in 0..3 {
            run(setup, &ops);
        }
    }
}

mod workloads {
    use spike_ecs_harness::{Params, Workload, check_conformance};

    use crate::add_remove::{GroupedAddRemove, SparseSetAddRemove};
    use crate::agents::{GroupedAgents, SparseSetAgents};
    use crate::entity_churn::{GroupedEntityChurn, SparseSetEntityChurn};
    use crate::frag_iter::SparseSetFragIter;
    use crate::heavy_compute::{GroupedHeavyCompute, SparseSetHeavyCompute};
    use crate::simple_iter::{GroupedSimpleIter, SparseSetSimpleIter};
    use crate::sprites::{GroupedSprites, SparseSetSprites};
    use spike_ecs_vec::add_remove::VecAddRemove;
    use spike_ecs_vec::agents::VecAgents;
    use spike_ecs_vec::entity_churn::VecEntityChurn;
    use spike_ecs_vec::frag_iter::VecFragIter;
    use spike_ecs_vec::heavy_compute::VecHeavyCompute;
    use spike_ecs_vec::simple_iter::VecSimpleIter;
    use spike_ecs_vec::sprites::VecSprites;

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const TICKS: u64 = 120;
    const AGENT_TICKS: u64 = 1_300;

    #[test]
    fn simple_iter_sparse_set_matches_the_reference() {
        check_conformance::<VecSimpleIter, SparseSetSimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn simple_iter_grouped_matches_the_reference() {
        check_conformance::<VecSimpleIter, GroupedSimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn frag_iter_sparse_set_matches_the_reference() {
        check_conformance::<VecFragIter, SparseSetFragIter>(
            Params {
                count: 20,
                seed: 42,
            },
            TICKS,
        )
        .unwrap();
    }

    #[test]
    fn heavy_compute_sparse_set_matches_the_reference() {
        check_conformance::<VecHeavyCompute, SparseSetHeavyCompute>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn heavy_compute_grouped_matches_the_reference() {
        check_conformance::<VecHeavyCompute, GroupedHeavyCompute>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn grouped_simple_iter_uses_the_group() {
        let w = GroupedSimpleIter::setup(Params { count: 10, seed: 1 });
        assert!(w.uses_group());
        let w = SparseSetSimpleIter::setup(Params { count: 10, seed: 1 });
        assert!(!w.uses_group());
    }

    #[test]
    fn grouped_heavy_compute_uses_the_group() {
        let w = GroupedHeavyCompute::setup(Params { count: 10, seed: 1 });
        assert!(w.uses_group());
        let w = SparseSetHeavyCompute::setup(Params { count: 10, seed: 1 });
        assert!(!w.uses_group());
    }

    #[test]
    fn add_remove_sparse_set_matches_the_reference() {
        check_conformance::<VecAddRemove, SparseSetAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_grouped_matches_the_reference() {
        check_conformance::<VecAddRemove, GroupedAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn entity_churn_sparse_set_matches_the_reference() {
        check_conformance::<VecEntityChurn, SparseSetEntityChurn>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn entity_churn_grouped_matches_the_reference() {
        check_conformance::<VecEntityChurn, GroupedEntityChurn>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn grouped_entity_churn_uses_the_group() {
        let w = GroupedEntityChurn::setup(Params { count: 10, seed: 1 });
        assert!(w.uses_group());
        let w = SparseSetEntityChurn::setup(Params { count: 10, seed: 1 });
        assert!(!w.uses_group());
    }

    #[test]
    fn grouped_add_remove_takes_the_view_path() {
        for count in [0, 10] {
            let w = GroupedAddRemove::setup(Params { count, seed: 1 });
            assert!(!w.uses_group_for_read_pass());
            assert!(!w.uses_group_for_remove_pass());
        }
        let w = SparseSetAddRemove::setup(Params { count: 10, seed: 1 });
        assert!(!w.uses_group_for_read_pass());
        assert!(!w.uses_group_for_remove_pass());
    }

    fn churn_keeps_invariants<const GROUPED: bool>() {
        let mut w = crate::entity_churn::EntityChurnOn::<GROUPED>::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
        }
    }

    #[test]
    fn entity_churn_keeps_invariants_every_tick() {
        churn_keeps_invariants::<false>();
        churn_keeps_invariants::<true>();
    }

    #[test]
    fn sprites_sparse_set_matches_the_reference() {
        check_conformance::<VecSprites, SparseSetSprites>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn sprites_grouped_matches_the_reference() {
        check_conformance::<VecSprites, GroupedSprites>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn agents_sparse_set_matches_the_reference() {
        check_conformance::<VecAgents, SparseSetAgents>(PARAMS, AGENT_TICKS).unwrap();
    }

    #[test]
    fn agents_grouped_matches_the_reference() {
        check_conformance::<VecAgents, GroupedAgents>(PARAMS, AGENT_TICKS).unwrap();
    }

    #[test]
    fn grouped_sprites_moves_on_the_group_path_and_draws_on_the_view_path() {
        for count in [0, 10] {
            let w = GroupedSprites::setup(Params { count, seed: 1 });
            assert!(w.uses_group_for_move());
            assert!(!w.uses_group_for_draw());
        }
        let w = SparseSetSprites::setup(Params { count: 10, seed: 1 });
        assert!(!w.uses_group_for_move());
        assert!(!w.uses_group_for_draw());
    }

    #[test]
    fn grouped_agents_uses_the_group() {
        let w = GroupedAgents::setup(Params { count: 10, seed: 1 });
        assert!(w.uses_group());
        let w = SparseSetAgents::setup(Params { count: 10, seed: 1 });
        assert!(!w.uses_group());
    }

    fn agents_keep_invariants<const GROUPED: bool>() {
        let mut w = crate::agents::AgentsOn::<GROUPED>::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..AGENT_TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
        }
    }

    #[test]
    fn agents_keeps_invariants_every_tick() {
        agents_keep_invariants::<false>();
        agents_keep_invariants::<true>();
    }

    fn zero_count<W: Workload>() {
        let mut w = W::setup(Params { count: 0, seed: 7 });
        assert_eq!(w.checksum(), 0);
        w.step();
        assert_eq!(w.checksum(), 0);
    }

    #[test]
    fn zero_count_is_valid() {
        zero_count::<SparseSetSimpleIter>();
        zero_count::<GroupedSimpleIter>();
        zero_count::<SparseSetFragIter>();
        zero_count::<SparseSetHeavyCompute>();
        zero_count::<GroupedHeavyCompute>();
        zero_count::<SparseSetAddRemove>();
        zero_count::<GroupedAddRemove>();
        zero_count::<SparseSetEntityChurn>();
        zero_count::<GroupedEntityChurn>();
        zero_count::<SparseSetSprites>();
        zero_count::<GroupedSprites>();
        zero_count::<SparseSetAgents>();
        zero_count::<GroupedAgents>();
    }
}
