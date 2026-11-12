//! Model-based property tests: `HandlePool` against a `BTreeMap` reference.

use std::collections::BTreeMap;

use engine_handle::{Handle, HandlePool};
use proptest::prelude::*;

/// WASI has no writable directory for proptest's failure persistence.
fn config() -> ProptestConfig {
    if cfg!(target_os = "wasi") {
        ProptestConfig {
            failure_persistence: None,
            ..ProptestConfig::default()
        }
    } else {
        ProptestConfig::default()
    }
}

#[derive(Debug, Clone)]
enum Op {
    Insert(u16),
    Remove(usize),
    Get(usize),
    GetDisjoint(usize, usize),
    /// Keep an element if `value % k != 0`.
    Retain(u16),
    Clear,
    Iterate,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => any::<u16>().prop_map(Op::Insert),
        3 => any::<usize>().prop_map(Op::Remove),
        2 => any::<usize>().prop_map(Op::Get),
        2 => (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::GetDisjoint(a, b)),
        1 => (1_u16..8).prop_map(Op::Retain),
        1 => Just(Op::Clear),
        1 => Just(Op::Iterate),
    ]
}

type Snapshot = Vec<(u64, u16)>;

/// Runs `ops` against the pool and the model, checking every invariant after each op.
/// Returns every handle issued and the `iter()` order after each step.
fn run(ops: &[Op]) -> (Vec<u64>, Vec<Snapshot>) {
    let mut pool = HandlePool::<u16>::new();
    let mut model = BTreeMap::<u64, u16>::new();
    let mut issued: Vec<Handle<u16>> = Vec::new();
    let mut trace = Vec::new();

    for op in ops {
        let pick = |i: usize| (!issued.is_empty()).then(|| issued[i % issued.len()]);
        match *op {
            Op::Insert(v) => {
                let h = pool.insert(v);
                assert!(
                    model.insert(h.to_bits(), v).is_none(),
                    "handle {h:?} issued twice"
                );
                issued.push(h);
            }
            Op::Remove(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(pool.remove(h), model.remove(&h.to_bits()));
                }
            }
            Op::Get(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(pool.get(h).copied(), model.get(&h.to_bits()).copied());
                }
            }
            Op::GetDisjoint(i, j) => {
                if let (Some(a), Some(b)) = (pick(i), pick(j)) {
                    let expect = a != b
                        && model.contains_key(&a.to_bits())
                        && model.contains_key(&b.to_bits());
                    let got = pool.get_disjoint_mut([a, b]);
                    assert_eq!(got.is_some(), expect);
                    if let Some([x, y]) = got {
                        *x = x.wrapping_add(1);
                        *y = y.wrapping_add(2);
                        *model.get_mut(&a.to_bits()).expect("a is live") = *x;
                        *model.get_mut(&b.to_bits()).expect("b is live") = *y;
                    }
                }
            }
            Op::Retain(k) => {
                pool.retain(|h, v| {
                    assert_eq!(model.get(&h.to_bits()), Some(&*v));
                    *v % k != 0
                });
                model.retain(|_, v| *v % k != 0);
            }
            Op::Clear => {
                pool.clear();
                model.clear();
            }
            Op::Iterate => {}
        }

        pool.assert_invariants();
        assert_eq!(pool.len(), model.len());
        let snapshot: Snapshot = pool.iter().map(|(h, v)| (h.to_bits(), *v)).collect();
        let as_model: BTreeMap<_, _> = snapshot.iter().copied().collect();
        assert_eq!(as_model, model);
        assert!(
            pool.values().iter().eq(snapshot.iter().map(|(_, v)| v)),
            "values() order != iter() order"
        );
        for h in &issued {
            assert_eq!(pool.contains(*h), model.contains_key(&h.to_bits()));
        }
        trace.push(snapshot);
    }
    (issued.iter().map(|h| h.to_bits()).collect(), trace)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn pool_matches_model_and_is_deterministic(ops in proptest::collection::vec(op(), 0..200)) {
        let first = run(&ops);
        let second = run(&ops);
        prop_assert_eq!(first, second); // same handles AND same iteration order
    }
}
