//! Model-based property tests: `GenIdMap` against a `BTreeMap` reference.
#![forbid(unsafe_code)]

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use engine::genid::{GenId, GenIdAllocator, GenIdMap, UntypedGenId};
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

/// Runs `ops` against the map and the model, checking every invariant after each op.
/// Returns every ID issued and the `iter()` order after each step.
fn run(ops: &[Op]) -> (Vec<u64>, Vec<Snapshot>) {
    let mut map = GenIdMap::<u16>::new();
    let mut model = BTreeMap::<u64, u16>::new();
    let mut issued: Vec<GenId<u16>> = Vec::new();
    let mut trace = Vec::new();

    for op in ops {
        let pick = |i: usize| (!issued.is_empty()).then(|| issued[i % issued.len()]);
        match *op {
            Op::Insert(v) => {
                let h = map.insert(v);
                assert!(
                    model.insert(h.to_bits(), v).is_none(),
                    "ID {h:?} issued twice"
                );
                issued.push(h);
            }
            Op::Remove(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(map.remove(h), model.remove(&h.to_bits()));
                }
            }
            Op::Get(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(map.get(h).copied(), model.get(&h.to_bits()).copied());
                }
            }
            Op::GetDisjoint(i, j) => {
                if let (Some(a), Some(b)) = (pick(i), pick(j)) {
                    let expect = a != b
                        && model.contains_key(&a.to_bits())
                        && model.contains_key(&b.to_bits());
                    let got = map.get_disjoint_mut([a, b]);
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
                map.retain(|h, v| {
                    assert_eq!(model.get(&h.to_bits()), Some(&*v));
                    *v % k != 0
                });
                model.retain(|_, v| *v % k != 0);
            }
            Op::Clear => {
                map.clear();
                model.clear();
            }
            Op::Iterate => {}
        }

        map.check_invariants().expect("invariants hold");
        assert_eq!(map.len(), model.len());
        let snapshot: Snapshot = map.iter().map(|(h, v)| (h.to_bits(), *v)).collect();
        let as_model: BTreeMap<_, _> = snapshot.iter().copied().collect();
        assert_eq!(as_model, model);
        assert!(
            map.values().iter().eq(snapshot.iter().map(|(_, v)| v)),
            "values() order != iter() order"
        );
        for h in &issued {
            assert_eq!(map.contains(*h), model.contains_key(&h.to_bits()));
        }
        trace.push(snapshot);
    }
    (issued.iter().map(|h| h.to_bits()).collect(), trace)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn map_matches_model_and_is_deterministic(ops in proptest::collection::vec(op(), 0..200)) {
        let first = run(&ops);
        let second = run(&ops);
        prop_assert_eq!(first, second); // same IDs AND same iteration order
    }
}

#[derive(Debug, Clone)]
enum AllocOp {
    Allocate,
    /// Allocate this many IDs in one batch (lean allocator) or a loop (unit map).
    AllocateMany(u8),
    Free(usize),
    IsAlive(usize),
    /// Keep an ID if `index % k != 0`.
    Retain(u32),
    Clear,
}

fn alloc_op() -> impl Strategy<Value = AllocOp> {
    prop_oneof![
        4 => Just(AllocOp::Allocate),
        1 => (0_u8..40).prop_map(AllocOp::AllocateMany),
        3 => any::<usize>().prop_map(AllocOp::Free),
        2 => any::<usize>().prop_map(AllocOp::IsAlive),
        1 => (1_u32..8).prop_map(AllocOp::Retain),
        1 => Just(AllocOp::Clear),
    ]
}

/// Runs `ops` against a `GenIdMap<()>` (the map as an iterable ID allocator) and a
/// `BTreeSet` model; returns every ID issued and the `iter()` order after each step.
fn run_alloc(ops: &[AllocOp]) -> (Vec<u64>, Vec<Vec<u64>>) {
    let mut alloc = GenIdMap::<()>::new();
    let mut model = BTreeSet::<u64>::new();
    let mut issued: Vec<GenId<()>> = Vec::new();
    let mut trace = Vec::new();

    for op in ops {
        let pick = |i: usize| (!issued.is_empty()).then(|| issued[i % issued.len()]);
        match *op {
            AllocOp::Allocate => {
                let h = alloc.insert(());
                assert!(model.insert(h.to_bits()), "ID {h:?} issued twice");
                issued.push(h);
            }
            AllocOp::AllocateMany(n) => {
                for _ in 0..n {
                    let h = alloc.insert(());
                    assert!(model.insert(h.to_bits()), "ID {h:?} issued twice");
                    issued.push(h);
                }
            }
            AllocOp::Free(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(alloc.remove(h).is_some(), model.remove(&h.to_bits()));
                }
            }
            AllocOp::IsAlive(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(alloc.contains(h), model.contains(&h.to_bits()));
                }
            }
            AllocOp::Retain(k) => {
                alloc.retain(|h, ()| h.index() % k != 0);
                model.retain(|bits| u32::try_from(bits >> 32).expect("index") % k != 0);
            }
            AllocOp::Clear => {
                alloc.clear();
                model.clear();
            }
        }

        alloc.check_invariants().expect("invariants hold");
        assert_eq!(alloc.len(), model.len());
        let snapshot: Vec<u64> = alloc.iter().map(|(h, ())| h.to_bits()).collect();
        assert_eq!(snapshot.iter().copied().collect::<BTreeSet<_>>(), model);
        for h in &issued {
            assert_eq!(alloc.contains(*h), model.contains(&h.to_bits()));
        }
        trace.push(snapshot);
    }
    (issued.iter().map(|h| h.to_bits()).collect(), trace)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn unit_map_matches_model_and_is_deterministic(ops in proptest::collection::vec(alloc_op(), 0..200)) {
        let first = run_alloc(&ops);
        let second = run_alloc(&ops);
        prop_assert_eq!(first, second);
    }
}

/// Runs `ops` (without `Retain`, which the lean allocator lacks) against the lean allocator,
/// a `GenIdMap<()>` and a set model. Both must issue identical IDs and agree on liveness
/// after every step.
fn run_lean_against_unit_map(ops: &[AllocOp]) {
    let mut lean = GenIdAllocator::<()>::new();
    let mut unit = GenIdMap::<()>::new();
    let mut model = BTreeSet::<u64>::new();
    let mut issued: Vec<GenId<()>> = Vec::new();

    for op in ops {
        let pick = |i: usize| (!issued.is_empty()).then(|| issued[i % issued.len()]);
        match *op {
            AllocOp::Allocate => {
                let (a, b) = (lean.allocate(), unit.insert(()));
                assert_eq!(a, b, "the allocator and the map issued different IDs");
                assert!(model.insert(a.to_bits()), "ID {a:?} issued twice");
                issued.push(a);
            }
            AllocOp::AllocateMany(n) => {
                let batch = lean.allocate_many(usize::from(n));
                for a in batch {
                    let b = unit.insert(());
                    assert_eq!(a, b, "batch and loop issued different IDs");
                    assert!(model.insert(a.to_bits()), "ID {a:?} issued twice");
                    issued.push(a);
                }
            }
            AllocOp::Free(i) => {
                if let Some(h) = pick(i) {
                    let was_live = model.remove(&h.to_bits());
                    assert_eq!(lean.free(h), was_live);
                    assert_eq!(unit.remove(h).is_some(), was_live);
                }
            }
            AllocOp::IsAlive(_) | AllocOp::Retain(_) => {}
            AllocOp::Clear => {
                lean.clear();
                unit.clear();
                model.clear();
            }
        }
        lean.check_invariants().expect("invariants hold");
        assert_eq!(lean.len(), model.len());
        assert_eq!(lean.len(), unit.len());
        for h in &issued {
            assert_eq!(lean.is_alive(*h), model.contains(&h.to_bits()));
        }
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn lean_allocator_matches_unit_map_and_model(ops in proptest::collection::vec(alloc_op(), 0..200)) {
        run_lean_against_unit_map(&ops);
    }
}

proptest! {
    #![proptest_config(config())]

    /// `to_bits` and `from_bits` are inverse for every `u64` that is a valid ID, and
    /// `from_bits` rejects exactly the values whose generation (low) half is zero.
    #[test]
    fn bits_roundtrip(bits in any::<u64>()) {
        if let Some(id) = GenId::<u8>::from_bits(bits) {
            prop_assert_ne!(bits & 0xFFFF_FFFF, 0);
            prop_assert_eq!(id.to_bits(), bits);
            prop_assert_eq!(u64::from(id.index()), bits >> 32);
        } else {
            prop_assert_eq!(bits & 0xFFFF_FFFF, 0);
        }
    }
}

proptest! {
    #![proptest_config(config())]

    /// The typed and untyped constructors accept the same bit patterns and keep them.
    #[test]
    fn typed_and_untyped_agree(bits in any::<u64>()) {
        prop_assert_eq!(
            GenId::<u8>::from_bits(bits).map(GenId::untyped),
            UntypedGenId::from_bits(bits)
        );
        if let Some(untyped) = UntypedGenId::from_bits(bits) {
            let typed = GenId::<u8>::from_untyped(untyped);
            prop_assert_eq!(typed.untyped(), untyped);
            prop_assert_eq!(typed.to_bits(), untyped.to_bits());
            prop_assert_eq!(untyped.to_bits(), bits);
        }
    }

    /// Comparing, equality and hashing give the same answers typed and untyped.
    #[test]
    fn typed_and_untyped_compare_and_hash_alike(a in any::<u64>(), b in any::<u64>()) {
        let (a, b) = (a | 1, b | 1);
        let (ta, tb) = (GenId::<u8>::from_bits(a).unwrap(), GenId::<u8>::from_bits(b).unwrap());
        let (ua, ub) = (ta.untyped(), tb.untyped());
        prop_assert_eq!(ta.cmp(&tb), ua.cmp(&ub));
        prop_assert_eq!(ta.cmp(&tb), a.cmp(&b));
        prop_assert_eq!(ta == tb, ua == ub);
        let hash = |value: &dyn Fn(&mut DefaultHasher)| {
            let mut hasher = DefaultHasher::new();
            value(&mut hasher);
            hasher.finish()
        };
        prop_assert_eq!(hash(&|h| ta.hash(h)), hash(&|h| ua.hash(h)));
        prop_assert_eq!(hash(&|h| tb.hash(h)), hash(&|h| ub.hash(h)));
    }
}
