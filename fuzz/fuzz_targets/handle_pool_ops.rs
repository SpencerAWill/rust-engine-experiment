//! Fuzzes `HandlePool` against a `BTreeMap` reference model, checking the pool's internal
//! invariants and its drop accounting after every operation.

#![no_main]
#![forbid(unsafe_code)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use arbitrary::Arbitrary;
use engine_handle::{Handle, HandlePool};
use libfuzzer_sys::fuzz_target;

/// The pool's value type: counts its own drops in a counter shared with the harness, so a value
/// that is leaked or dropped twice breaks the accounting equation.
struct Tracked {
    value: u16,
    drops: Rc<Cell<u32>>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(u16),
    TryInsert(u16),
    Remove(usize),
    Get(usize),
    GetMut(usize, u16),
    GetDisjoint(usize, usize),
    /// Keeps a value when `value % (k % 8 + 1) != 0`.
    Retain(u16),
    Drain,
    Clear,
    Iterate,
    /// A handle built from arbitrary bits, which the pool probably never issued.
    Probe(u64),
    /// An issued handle's slot with a nearby generation (a stale or never-issued neighbour).
    ProbeNear(usize, i8),
}

#[derive(Arbitrary, Debug)]
struct Input {
    /// `None` is an unbounded pool; `Some(c)` is a fixed capacity of `c % 17`.
    capacity: Option<u8>,
    ops: Vec<Op>,
}

/// The pool under test, its model, and the bookkeeping the checks need.
struct Harness {
    pool: HandlePool<Tracked>,
    /// Live handles by `to_bits`, with their values. Derived only from the ops applied so far.
    model: BTreeMap<u64, u16>,
    /// The fixed capacity, if any.
    limit: Option<usize>,
    /// Every handle the pool has issued, for modulo selection (stale ones included).
    issued: Vec<Handle<Tracked>>,
    /// `to_bits` of every handle issued so far: no pair may ever be issued twice.
    ever_issued: BTreeSet<u64>,
    drops: Rc<Cell<u32>>,
    /// How many `Tracked` values have been constructed, including rejected ones.
    created: u32,
}

impl Harness {
    fn new(capacity: Option<u8>) -> Self {
        let limit = capacity.map(|c| usize::from(c % 17));
        Self {
            pool: limit.map_or_else(HandlePool::new, HandlePool::with_fixed_capacity),
            model: BTreeMap::new(),
            limit,
            issued: Vec::new(),
            ever_issued: BTreeSet::new(),
            drops: Rc::new(Cell::new(0)),
            created: 0,
        }
    }

    fn make(&mut self, value: u16) -> Tracked {
        self.created += 1;
        Tracked {
            value,
            drops: Rc::clone(&self.drops),
        }
    }

    fn at_limit(&self) -> bool {
        self.limit.is_some_and(|limit| self.model.len() >= limit)
    }

    /// An issued handle chosen by `i`, or `None` before the first insert.
    fn pick(&self, i: usize) -> Option<Handle<Tracked>> {
        (!self.issued.is_empty()).then(|| self.issued[i % self.issued.len()])
    }

    /// Records a handle the pool just issued for `value`.
    fn register(&mut self, handle: Handle<Tracked>, value: u16) {
        let bits = handle.to_bits();
        assert!(self.ever_issued.insert(bits), "{handle:?} was issued twice");
        assert!(
            self.model.insert(bits, value).is_none(),
            "{handle:?} was already live"
        );
        self.issued.push(handle);
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Insert(value) => {
                // `insert` panics at the limit, as documented; `TryInsert` covers that path.
                if !self.at_limit() {
                    let tracked = self.make(value);
                    let handle = self.pool.insert(tracked);
                    self.register(handle, value);
                }
            }
            Op::TryInsert(value) => {
                let was_full = self.at_limit();
                let tracked = self.make(value);
                match self.pool.try_insert(tracked) {
                    Ok(handle) => {
                        assert!(!was_full, "inserted into a full pool");
                        self.register(handle, value);
                    }
                    Err(error) => {
                        assert!(was_full, "rejected an insert below the limit");
                        assert_eq!(Some(error.limit() as usize), self.limit);
                        assert_eq!(error.into_inner().value, value);
                    }
                }
            }
            Op::Remove(i) => {
                if let Some(handle) = self.pick(i) {
                    let expected = self.model.remove(&handle.to_bits());
                    assert_eq!(self.pool.remove(handle).map(|t| t.value), expected);
                }
            }
            Op::Get(i) => {
                if let Some(handle) = self.pick(i) {
                    let expected = self.model.get(&handle.to_bits()).copied();
                    assert_eq!(self.pool.get(handle).map(|t| t.value), expected);
                    assert_eq!(self.pool.contains(handle), expected.is_some());
                }
            }
            Op::GetMut(i, value) => {
                if let Some(handle) = self.pick(i) {
                    match (
                        self.pool.get_mut(handle),
                        self.model.get_mut(&handle.to_bits()),
                    ) {
                        (Some(tracked), Some(modelled)) => {
                            assert_eq!(tracked.value, *modelled);
                            tracked.value = value;
                            *modelled = value;
                        }
                        (None, None) => {}
                        (got, expected) => panic!(
                            "get_mut disagrees for {handle:?}: pool {}, model {}",
                            got.is_some(),
                            expected.is_some()
                        ),
                    }
                }
            }
            Op::GetDisjoint(i, j) => {
                if let (Some(a), Some(b)) = (self.pick(i), self.pick(j)) {
                    let (abits, bbits) = (a.to_bits(), b.to_bits());
                    let expect = a != b
                        && self.model.contains_key(&abits)
                        && self.model.contains_key(&bbits);
                    match self.pool.get_disjoint_mut([a, b]) {
                        Some([x, y]) => {
                            assert!(expect, "get_disjoint_mut succeeded for {a:?}, {b:?}");
                            std::mem::swap(&mut x.value, &mut y.value);
                            let (va, vb) = (self.model[&abits], self.model[&bbits]);
                            self.model.insert(abits, vb);
                            self.model.insert(bbits, va);
                        }
                        None => assert!(!expect, "get_disjoint_mut failed for {a:?}, {b:?}"),
                    }
                }
            }
            Op::Retain(k) => {
                let k = k % 8 + 1;
                let model = &self.model;
                self.pool.retain(|handle, tracked| {
                    assert_eq!(model.get(&handle.to_bits()), Some(&tracked.value));
                    tracked.value % k != 0
                });
                self.model.retain(|_, value| *value % k != 0);
            }
            Op::Drain => {
                let drained: Vec<(Handle<Tracked>, Tracked)> = self.pool.drain().collect();
                let seen: BTreeMap<u64, u16> = drained
                    .iter()
                    .map(|(handle, tracked)| (handle.to_bits(), tracked.value))
                    .collect();
                assert_eq!(seen.len(), drained.len(), "drain yielded a handle twice");
                assert_eq!(seen, self.model);
                drop(drained);
                self.model.clear();
            }
            Op::Clear => {
                self.pool.clear();
                self.model.clear();
            }
            Op::Iterate => {
                // `check` compares `iter()`; this compares the dense value slice too.
                let mut values: Vec<u16> = self.pool.values().iter().map(|t| t.value).collect();
                let mut expected: Vec<u16> = self.model.values().copied().collect();
                values.sort_unstable();
                expected.sort_unstable();
                assert_eq!(values, expected);
            }
            Op::Probe(bits) => {
                if let Some(handle) = Handle::<Tracked>::from_bits(bits) {
                    self.probe(handle);
                }
            }
            Op::ProbeNear(i, delta) => {
                if let Some(issued) = self.pick(i) {
                    let generation = issued
                        .generation()
                        .get()
                        .wrapping_add_signed(i32::from(delta));
                    let bits = (u64::from(issued.index()) << 32) | u64::from(generation);
                    if let Some(handle) = Handle::<Tracked>::from_bits(bits) {
                        self.probe(handle);
                    }
                }
            }
        }
    }

    /// A handle the model does not hold must be reported absent everywhere, without panicking
    /// and without changing the pool (`check` verifies the latter).
    fn probe(&mut self, handle: Handle<Tracked>) {
        if self.model.contains_key(&handle.to_bits()) {
            return;
        }
        assert!(!self.pool.contains(handle), "{handle:?} is not live");
        assert!(self.pool.get(handle).is_none());
        assert!(self.pool.get_mut(handle).is_none());
        assert!(self.pool.remove(handle).is_none());
    }

    /// Everything that must hold after every operation.
    fn check(&self) {
        self.pool
            .check_invariants()
            .expect("the pool's invariants must hold");
        assert_eq!(self.pool.len(), self.model.len());
        assert_eq!(self.pool.is_empty(), self.model.is_empty());

        let snapshot = |pool: &HandlePool<Tracked>| -> Vec<(u64, u16)> {
            pool.iter()
                .map(|(handle, tracked)| (handle.to_bits(), tracked.value))
                .collect()
        };
        let first = snapshot(&self.pool);
        // The dense order is an implementation detail, but it must be stable between calls.
        assert_eq!(first, snapshot(&self.pool));
        assert_eq!(self.pool.iter().len(), first.len());
        let as_set: BTreeMap<u64, u16> = first.iter().copied().collect();
        assert_eq!(as_set.len(), first.len(), "iter() yielded a handle twice");
        assert_eq!(as_set, self.model);

        // Every value ever built is either live or has been dropped exactly once.
        let live = u32::try_from(self.model.len()).expect("a model this small fits in u32");
        assert_eq!(
            self.created,
            live + self.drops.get(),
            "values leaked or double-dropped"
        );
    }

    /// Drops the pool and checks that nothing was leaked.
    fn finish(self) {
        let Self {
            pool,
            drops,
            created,
            ..
        } = self;
        drop(pool);
        assert_eq!(
            created,
            drops.get(),
            "values leaked when the pool was dropped"
        );
    }
}

fuzz_target!(|input: Input| {
    let mut harness = Harness::new(input.capacity);
    for op in &input.ops {
        harness.apply(op);
        harness.check();
    }
    harness.finish();
});
