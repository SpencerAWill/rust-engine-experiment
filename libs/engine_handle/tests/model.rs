//! Model-based property tests: `HandlePool` against a `BTreeMap` reference.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use engine_handle::{DenseHandleAllocator, Handle, HandleAllocator, HandlePool};
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

        pool.check_invariants().expect("invariants hold");
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

#[derive(Debug, Clone)]
enum AllocOp {
    Allocate,
    /// Allocate this many handles in one batch (lean allocator) or a loop (dense).
    AllocateMany(u8),
    Free(usize),
    IsAlive(usize),
    /// Keep a handle if `index % k != 0`.
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

/// Runs `ops` against the allocator and a `BTreeSet` model; returns every handle issued
/// and the `iter()` order after each step.
fn run_alloc(ops: &[AllocOp]) -> (Vec<u64>, Vec<Vec<u64>>) {
    let mut alloc = DenseHandleAllocator::<u8>::new();
    let mut model = BTreeSet::<u64>::new();
    let mut issued: Vec<Handle<u8>> = Vec::new();
    let mut trace = Vec::new();

    for op in ops {
        let pick = |i: usize| (!issued.is_empty()).then(|| issued[i % issued.len()]);
        match *op {
            AllocOp::Allocate => {
                let h = alloc.allocate();
                assert!(model.insert(h.to_bits()), "handle {h:?} issued twice");
                issued.push(h);
            }
            AllocOp::AllocateMany(n) => {
                for _ in 0..n {
                    let h = alloc.allocate();
                    assert!(model.insert(h.to_bits()), "handle {h:?} issued twice");
                    issued.push(h);
                }
            }
            AllocOp::Free(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(alloc.free(h), model.remove(&h.to_bits()));
                }
            }
            AllocOp::IsAlive(i) => {
                if let Some(h) = pick(i) {
                    assert_eq!(alloc.is_alive(h), model.contains(&h.to_bits()));
                }
            }
            AllocOp::Retain(k) => {
                alloc.retain(|h| h.index() % k != 0);
                model.retain(|bits| u32::try_from(bits >> 32).expect("index") % k != 0);
            }
            AllocOp::Clear => {
                alloc.clear();
                model.clear();
            }
        }

        alloc.check_invariants().expect("invariants hold");
        assert_eq!(alloc.len(), model.len());
        let snapshot: Vec<u64> = alloc.iter().map(Handle::to_bits).collect();
        assert_eq!(snapshot.iter().copied().collect::<BTreeSet<_>>(), model);
        for h in &issued {
            assert_eq!(alloc.is_alive(*h), model.contains(&h.to_bits()));
        }
        trace.push(snapshot);
    }
    (issued.iter().map(|h| h.to_bits()).collect(), trace)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn allocator_matches_model_and_is_deterministic(ops in proptest::collection::vec(alloc_op(), 0..200)) {
        let first = run_alloc(&ops);
        let second = run_alloc(&ops);
        prop_assert_eq!(first, second);
    }
}

/// Runs `ops` (without `Retain`, which the lean allocator lacks) against the lean allocator,
/// the dense allocator and a set model. Both allocators must issue identical handles and agree
/// on liveness after every step.
fn run_lean_against_dense(ops: &[AllocOp]) {
    let mut lean = HandleAllocator::<u8>::new();
    let mut dense = DenseHandleAllocator::<u8>::new();
    let mut model = BTreeSet::<u64>::new();
    let mut issued: Vec<Handle<u8>> = Vec::new();

    for op in ops {
        let pick = |i: usize| (!issued.is_empty()).then(|| issued[i % issued.len()]);
        match *op {
            AllocOp::Allocate => {
                let (a, b) = (lean.allocate(), dense.allocate());
                assert_eq!(a, b, "allocators issued different handles");
                assert!(model.insert(a.to_bits()), "handle {a:?} issued twice");
                issued.push(a);
            }
            AllocOp::AllocateMany(n) => {
                let batch = lean.allocate_many(usize::from(n));
                for a in batch {
                    let b = dense.allocate();
                    assert_eq!(a, b, "batch and loop issued different handles");
                    assert!(model.insert(a.to_bits()), "handle {a:?} issued twice");
                    issued.push(a);
                }
            }
            AllocOp::Free(i) => {
                if let Some(h) = pick(i) {
                    let was_live = model.remove(&h.to_bits());
                    assert_eq!(lean.free(h), was_live);
                    assert_eq!(dense.free(h), was_live);
                }
            }
            AllocOp::IsAlive(_) | AllocOp::Retain(_) => {}
            AllocOp::Clear => {
                lean.clear();
                dense.clear();
                model.clear();
            }
        }
        lean.check_invariants().expect("invariants hold");
        assert_eq!(lean.len(), model.len());
        assert_eq!(lean.len(), dense.len());
        for h in &issued {
            assert_eq!(lean.is_alive(*h), model.contains(&h.to_bits()));
        }
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn lean_allocator_matches_dense_allocator_and_model(ops in proptest::collection::vec(alloc_op(), 0..200)) {
        run_lean_against_dense(&ops);
    }
}

proptest! {
    #![proptest_config(config())]

    /// `to_bits` and `from_bits` are inverse for every `u64` that is a valid handle, and
    /// `from_bits` rejects exactly the values whose generation (low) half is zero.
    #[test]
    fn bits_roundtrip(bits in any::<u64>()) {
        if let Some(handle) = Handle::<u8>::from_bits(bits) {
            prop_assert_ne!(bits & 0xFFFF_FFFF, 0);
            prop_assert_eq!(handle.to_bits(), bits);
            prop_assert_eq!(u64::from(handle.index()), bits >> 32);
        } else {
            prop_assert_eq!(bits & 0xFFFF_FFFF, 0);
        }
    }
}
